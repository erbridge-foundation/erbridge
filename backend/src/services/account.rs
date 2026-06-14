use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    audit::{self, AuditEvent},
    db::{accounts, characters, sessions},
    dto::account::TokenStatus,
    error::{AppError, ConflictKind},
    esi,
};

pub struct CharacterInfo {
    pub id: Uuid,
    pub eve_character_id: i64,
    pub name: String,
    pub corporation_id: i64,
    pub corporation_name: String,
    pub alliance_id: Option<i64>,
    pub alliance_name: Option<String>,
    pub is_main: bool,
    pub portrait_url: String,
    pub token_status: TokenStatus,
}

fn character_to_info(c: characters::Character) -> CharacterInfo {
    let portrait_url = esi::portrait_url(c.eve_character_id);
    let token_status = TokenStatus::from_db(&c.token_status);
    CharacterInfo {
        id: c.id,
        eve_character_id: c.eve_character_id,
        name: c.name,
        corporation_id: c.corporation_id,
        corporation_name: c.corporation_name,
        alliance_id: c.alliance_id,
        alliance_name: c.alliance_name,
        is_main: c.is_main,
        portrait_url,
        token_status,
    }
}

pub struct MeInfo {
    pub account: accounts::Account,
    pub characters: Vec<CharacterInfo>,
}

pub async fn get_me(pool: &PgPool, account_id: Uuid) -> Result<MeInfo, AppError> {
    let account = accounts::get_account(pool, account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let chars = characters::list_for_account(pool, account_id).await?;

    Ok(MeInfo {
        account,
        characters: chars.into_iter().map(character_to_info).collect(),
    })
}

pub async fn set_main_character(
    pool: &PgPool,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<CharacterInfo, AppError> {
    // Verify ownership and capture the new main's EVE ID + name for the audit
    // payload (snapshotted so the row stays readable after a later rename/delete).
    let (new_main_eve_id, new_main_name) =
        match characters::lookup_for_account(pool, character_id).await? {
            Some((owner_id, eve_id, name, _)) if owner_id == account_id => (eve_id, name),
            _ => return Err(AppError::NotFound),
        };

    let mut tx = pool.begin().await?;
    // Emit the audit row *before* the is_main flip so the actor-character
    // snapshot resolves to the outgoing main.
    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::CharacterSetMain {
            account_id,
            eve_character_id: new_main_eve_id,
            character_name: new_main_name,
        },
    )
    .await?;
    // `set_main` returns the promoted row via RETURNING — no post-commit re-list.
    let updated = characters::set_main(&mut tx, account_id, character_id)
        .await?
        .ok_or(AppError::NotFound)?;
    tx.commit().await?;

    Ok(character_to_info(updated))
}

pub async fn delete_account(pool: &PgPool, account_id: Uuid) -> Result<(), AppError> {
    let account = accounts::get_account(pool, account_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut tx = pool.begin().await?;
    // Evaluate the last-admin guard inside the transaction, *before* the status
    // flip. `count_server_admins_tx` takes a `FOR UPDATE` lock on every active
    // admin row, so two concurrent deletes from the final two admins serialise
    // here (each must lock the same row set): the second blocks until the first
    // commits, then re-reads a count of 1 (only itself) and is refused. Locking
    // before flipping (rather than after) avoids a lock-ordering deadlock.
    if account.is_server_admin {
        let admin_count = accounts::count_server_admins_tx(&mut tx).await?;
        if admin_count <= 1 {
            return Err(AppError::Conflict(
                ConflictKind::CannotRemoveLastServerAdmin,
            ));
        }
    }

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::AccountDeletionRequested { account_id },
    )
    .await?;
    accounts::soft_delete(&mut tx, account_id).await?;

    characters::clear_tokens_for_account(&mut tx, account_id).await?;
    // Delete sessions in the same transaction: cookie-path auth enforces
    // soft-delete solely through session absence, so a soft-deleted account
    // must never retain a usable session under any partial-failure ordering.
    sessions::delete_for_account_in_tx(&mut tx, account_id).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn delete_character(
    pool: &PgPool,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    // Delete the row first, scoped to the owner (the WHERE subsumes the
    // ownership check) and returning is_main + the audit snapshot fields. A
    // missing/foreign row matches nothing → 404. Both invariant guards are then
    // evaluated against the post-delete state inside this same transaction, so
    // two concurrent deletes on a two-character account serialise on the row
    // locks their DELETEs take and cannot jointly empty the account.
    let (eve_character_id, character_name, was_main) =
        match characters::delete_character_owned_in_tx(&mut tx, account_id, character_id).await? {
            Some(v) => v,
            None => return Err(AppError::NotFound),
        };

    let remaining = characters::count_for_account_in_tx(&mut tx, account_id).await?;

    // Last character: removing it would leave the account without an identity.
    if remaining == 0 {
        return Err(AppError::Conflict(ConflictKind::CannotRemoveLastCharacter));
    }

    // Main character while siblings remain: caller must promote another first.
    if was_main {
        return Err(AppError::Conflict(ConflictKind::CannotRemoveMain));
    }

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::CharacterRemoved {
            account_id,
            eve_character_id,
            character_name,
        },
    )
    .await?;
    tx.commit().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_url_uses_shared_esi_helper() {
        // The service derives portraits from the one `esi::portrait_url` helper;
        // pin the URL the API contract exposes.
        assert_eq!(
            esi::portrait_url(12345),
            "https://images.evetech.net/characters/12345/portrait?size=128"
        );
    }

    #[test]
    fn token_status_maps_db_values() {
        assert_eq!(TokenStatus::from_db("valid"), TokenStatus::Active);
        assert_eq!(TokenStatus::from_db("token_expired"), TokenStatus::Expired);
        assert_eq!(
            TokenStatus::from_db("owner_mismatch"),
            TokenStatus::OwnerMismatch
        );
        // Unknown values fail safe to Expired.
        assert_eq!(TokenStatus::from_db("bogus"), TokenStatus::Expired);
    }

    #[sqlx::test]
    async fn delete_account_blocks_last_server_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (admin_id, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let err = delete_account(&pool, admin_id).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastServerAdmin)
        ));

        let account = accounts::get_account(&pool, admin_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.status, "active");
    }

    #[sqlx::test]
    async fn delete_account_allows_admin_when_another_admin_exists(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (second, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        sqlx::query!(
            "UPDATE account SET is_server_admin = TRUE WHERE id = $1",
            second
        )
        .execute(&pool)
        .await
        .unwrap();

        let char_id = insert_test_character_with_tokens(&pool, first, 1001).await;

        delete_account(&pool, first).await.unwrap();

        let account = accounts::get_account(&pool, first).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert_character_tokens_cleared(&pool, char_id).await;
    }

    #[sqlx::test]
    async fn delete_account_allows_non_admin(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let char_id = insert_test_character_with_tokens(&pool, user, 1002).await;

        delete_account(&pool, user).await.unwrap();

        let account = accounts::get_account(&pool, user).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert_character_tokens_cleared(&pool, char_id).await;
    }

    #[sqlx::test]
    async fn delete_account_is_atomic_on_account_with_characters(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let char_one = insert_test_character_with_tokens(&pool, user, 9001).await;
        let char_two = insert_test_character_with_tokens(&pool, user, 9002).await;

        delete_account(&pool, user).await.unwrap();

        let account = accounts::get_account(&pool, user).await.unwrap().unwrap();
        assert_eq!(account.status, "soft_deleted");
        assert!(account.delete_requested_at.is_some());
        assert_character_tokens_cleared(&pool, char_one).await;
        assert_character_tokens_cleared(&pool, char_two).await;
    }

    #[sqlx::test]
    async fn delete_account_deletes_sessions_in_same_tx(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        sessions::insert(&pool, "sess-1", user).await.unwrap();
        sessions::insert(&pool, "sess-2", user).await.unwrap();

        delete_account(&pool, user).await.unwrap();

        // The soft-delete commit also removed every session for the account, so
        // the cookie-auth path (which checks only session presence) can never
        // authenticate a soft-deleted account.
        assert!(
            sessions::list_ids_for_account(&pool, user)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn delete_account_last_admin_guard_rolls_back_sessions(pool: PgPool) {
        // A lone server admin's delete is refused; assert the whole transaction
        // — status flip *and* session deletion — rolls back together.
        let mut tx = pool.begin().await.unwrap();
        let (admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sessions::insert(&pool, "admin-sess", admin).await.unwrap();

        let err = delete_account(&pool, admin).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastServerAdmin)
        ));

        // Status untouched and the session survived the rollback.
        let account = accounts::get_account(&pool, admin).await.unwrap().unwrap();
        assert_eq!(account.status, "active");
        assert_eq!(
            sessions::list_ids_for_account(&pool, admin).await.unwrap(),
            vec!["admin-sess".to_string()]
        );
    }

    #[sqlx::test]
    async fn delete_account_concurrent_last_two_admins_keeps_one(pool: PgPool) {
        // Two active admins each delete concurrently. At most one may succeed;
        // the guard (count after the flip, inside the tx) must leave ≥1 active
        // admin. The row locks the soft-delete UPDATE takes serialise the two
        // transactions, so the second sees the first's pending/committed flip.
        let mut tx = pool.begin().await.unwrap();
        let (first, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (second, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        sqlx::query!(
            "UPDATE account SET is_server_admin = TRUE WHERE id = $1",
            second
        )
        .execute(&pool)
        .await
        .unwrap();

        let p1 = pool.clone();
        let p2 = pool.clone();
        let h1 = tokio::spawn(async move { delete_account(&p1, first).await });
        let h2 = tokio::spawn(async move { delete_account(&p2, second).await });
        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();

        // Exactly one succeeded and one was refused with the conflict.
        let successes = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
        assert_eq!(successes, 1, "exactly one delete succeeds");
        let conflicts = [&r1, &r2]
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Err(AppError::Conflict(
                        ConflictKind::CannotRemoveLastServerAdmin
                    ))
                )
            })
            .count();
        assert_eq!(conflicts, 1, "the loser gets a 409");

        // At least one active server admin remains.
        assert!(accounts::count_server_admins(&pool).await.unwrap() >= 1);
    }

    #[sqlx::test]
    async fn delete_character_concurrent_deletes_keep_one_with_main(pool: PgPool) {
        // Account with exactly two characters: A (main) and B (not main). Two
        // deletes race for B; the last-character guard (count inside the tx,
        // after the delete) must leave the account non-empty with a main.
        let mut tx = pool.begin().await.unwrap();
        let (_admin, _) = accounts::resolve_or_create(&mut tx, None, 1001)
            .await
            .unwrap();
        let (user, _) = accounts::resolve_or_create(&mut tx, None, 1002)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let main_id = insert_main_character(&pool, user, 5001).await;
        let alt_id = insert_test_character_with_tokens(&pool, user, 5002).await;

        // Two parallel deletes of the same non-main character B. One deletes the
        // row; the other finds nothing (404). The account keeps A.
        let p1 = pool.clone();
        let p2 = pool.clone();
        let h1 = tokio::spawn(async move { delete_character(&p1, user, alt_id).await });
        let h2 = tokio::spawn(async move { delete_character(&p2, user, alt_id).await });
        let _ = h1.await.unwrap();
        let _ = h2.await.unwrap();

        let chars = characters::list_for_account(&pool, user).await.unwrap();
        assert!(!chars.is_empty(), "account never emptied");
        assert!(chars.iter().any(|c| c.is_main), "a main survives");
        assert!(chars.iter().any(|c| c.id == main_id));
    }

    #[sqlx::test]
    async fn delete_character_last_character_rejected(pool: PgPool) {
        let user = accounts::create_account(&pool).await.unwrap();
        let only = insert_main_character(&pool, user, 6001).await;

        let err = delete_character(&pool, user, only).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveLastCharacter)
        ));
        // Rolled back: the row survives.
        assert_eq!(
            characters::list_for_account(&pool, user)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[sqlx::test]
    async fn delete_character_main_with_siblings_rejected(pool: PgPool) {
        let user = accounts::create_account(&pool).await.unwrap();
        let main_id = insert_main_character(&pool, user, 7001).await;
        let _alt = insert_test_character_with_tokens(&pool, user, 7002).await;

        let err = delete_character(&pool, user, main_id).await.unwrap_err();
        assert!(matches!(
            err,
            AppError::Conflict(ConflictKind::CannotRemoveMain)
        ));
        // Rolled back: the main survives.
        assert_eq!(
            characters::list_for_account(&pool, user)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[sqlx::test]
    async fn delete_character_foreign_is_not_found(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let other = accounts::create_account(&pool).await.unwrap();
        let owner_main = insert_main_character(&pool, owner, 8001).await;
        let _owner_alt = insert_test_character_with_tokens(&pool, owner, 8002).await;

        // `other` cannot delete `owner`'s character.
        let err = delete_character(&pool, other, owner_main)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound));
        assert_eq!(
            characters::list_for_account(&pool, owner)
                .await
                .unwrap()
                .len(),
            2
        );
    }

    /// Inserts a token-bearing character and promotes it to main.
    async fn insert_main_character(pool: &PgPool, account_id: Uuid, eve_character_id: i64) -> Uuid {
        let id = insert_test_character_with_tokens(pool, account_id, eve_character_id).await;
        let mut tx = pool.begin().await.unwrap();
        characters::promote_if_no_main(
            &mut tx,
            account_id,
            id,
            eve_character_id,
            &format!("Pilot {eve_character_id}"),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        id
    }

    async fn insert_test_character_with_tokens(
        pool: &PgPool,
        account_id: Uuid,
        eve_character_id: i64,
    ) -> Uuid {
        let row = sqlx::query!(
            r#"
            INSERT INTO eve_character (
                account_id, eve_character_id, name, corporation_id, corporation_name,
                esi_client_id, encrypted_access_token, encrypted_refresh_token,
                access_token_expires_at, scopes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            account_id,
            eve_character_id,
            format!("Pilot {eve_character_id}"),
            1_000_001_i64,
            "Test Corp",
            "test-client",
            &[1u8, 2, 3][..],
            &[4u8, 5, 6][..],
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["esi-skills.read_skills.v1".to_string()][..],
        )
        .fetch_one(pool)
        .await
        .unwrap();
        row.id
    }

    async fn assert_character_tokens_cleared(pool: &PgPool, character_id: Uuid) {
        let row = sqlx::query!(
            r#"
            SELECT encrypted_access_token, encrypted_refresh_token,
                   access_token_expires_at, scopes
            FROM eve_character WHERE id = $1
            "#,
            character_id
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(row.encrypted_access_token.is_none());
        assert!(row.encrypted_refresh_token.is_none());
        assert!(row.access_token_expires_at.is_none());
        assert!(row.scopes.is_empty());
    }
}
