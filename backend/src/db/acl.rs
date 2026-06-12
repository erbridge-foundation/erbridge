use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

/// A reusable, named access-control list owned by the account that created it.
/// No orphan-reaping: an ACL attached to no map simply persists.
#[derive(Debug, Clone)]
pub struct Acl {
    pub id: Uuid,
    pub name: String,
    pub owner_account_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Inserts an ACL owned by `owner_account_id`.
pub async fn insert_acl(
    tx: &mut Transaction<'_, Postgres>,
    owner_account_id: Uuid,
    name: &str,
) -> Result<Acl> {
    sqlx::query_as!(
        Acl,
        r#"
        INSERT INTO acl (name, owner_account_id)
        VALUES ($1, $2)
        RETURNING id, name, owner_account_id, created_at, updated_at
        "#,
        name,
        owner_account_id,
    )
    .fetch_one(&mut **tx)
    .await
    .context("failed to insert acl")
}

/// Fetches an ACL by id. Generic over the executor so the service can run the
/// ownership check inside the same transaction as the mutation it guards
/// (passing `&mut *tx`), while read-only callers pass the pool.
pub async fn find_acl_by_id<'e, E>(executor: E, id: Uuid) -> Result<Option<Acl>>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as!(
        Acl,
        r#"
        SELECT id, name, owner_account_id, created_at, updated_at
        FROM acl
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(executor)
    .await
    .context("failed to fetch acl by id")
}

/// Renames an ACL. Returns the updated row, or `None` if no ACL with that id
/// exists.
pub async fn update_acl_name(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    name: &str,
) -> Result<Option<Acl>> {
    sqlx::query_as!(
        Acl,
        r#"
        UPDATE acl
        SET name = $2, updated_at = now()
        WHERE id = $1
        RETURNING id, name, owner_account_id, created_at, updated_at
        "#,
        id,
        name,
    )
    .fetch_optional(&mut **tx)
    .await
    .context("failed to update acl name")
}

/// Hard-deletes an ACL. FK `ON DELETE CASCADE` removes its members and any
/// `map_acl` attachments. Returns `true` if a row was deleted.
pub async fn delete_acl(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<bool> {
    let result = sqlx::query!("DELETE FROM acl WHERE id = $1", id)
        .execute(&mut **tx)
        .await
        .context("failed to delete acl")?;
    Ok(result.rows_affected() > 0)
}

/// Returns the ACLs an account can manage: those it owns, plus those on which it
/// holds `manage` or `admin` via a direct `character` member entry whose
/// character belongs to the account. Ordered by name.
pub async fn find_acls_manageable_by_account(pool: &PgPool, account_id: Uuid) -> Result<Vec<Acl>> {
    sqlx::query_as!(
        Acl,
        r#"
        SELECT id, name, owner_account_id, created_at, updated_at
        FROM acl
        WHERE owner_account_id = $1
           OR EXISTS (
               SELECT 1
               FROM acl_member am
               JOIN eve_character ec ON ec.id = am.character_id
               WHERE am.acl_id = acl.id
                 AND am.member_type = 'character'
                 AND am.permission IN ('manage', 'admin')
                 AND ec.account_id = $1
           )
        ORDER BY name
        "#,
        account_id,
    )
    .fetch_all(pool)
    .await
    .context("failed to fetch manageable acls for account")
}

#[cfg(test)]
/// Inserts an ACL through a one-shot transaction — convenience for other db
/// modules' tests.
pub async fn insert_acl_for_test(pool: &PgPool, owner: Uuid, name: &str) -> Acl {
    let mut tx = pool.begin().await.unwrap();
    let a = insert_acl(&mut tx, owner, name).await.unwrap();
    tx.commit().await.unwrap();
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::insert_character;
    use crate::db::{accounts, acl_member};

    #[sqlx::test]
    async fn insert_then_find_returns_same_row(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "My ACL").await;

        let found = find_acl_by_id(&pool, a.id).await.unwrap().unwrap();
        assert_eq!(found.id, a.id);
        assert_eq!(found.name, "My ACL");
        assert_eq!(found.owner_account_id, Some(owner));
    }

    #[sqlx::test]
    async fn rename_updates_name(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "Old").await;

        let mut tx = pool.begin().await.unwrap();
        let renamed = update_acl_name(&mut tx, a.id, "New")
            .await
            .unwrap()
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(renamed.name, "New");
    }

    #[sqlx::test]
    async fn rename_missing_returns_none(pool: PgPool) {
        let mut tx = pool.begin().await.unwrap();
        let result = update_acl_name(&mut tx, Uuid::new_v4(), "x").await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn delete_cascades_members(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let a = insert_acl_for_test(&pool, owner, "Doomed").await;
        let char_id = insert_character(&pool, owner, 10, "Pilot").await;
        acl_member::add_member(
            &pool,
            a.id,
            "character",
            None,
            Some(char_id),
            "Pilot",
            "read",
        )
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let deleted = delete_acl(&mut tx, a.id).await.unwrap();
        tx.commit().await.unwrap();
        assert!(deleted);

        assert!(find_acl_by_id(&pool, a.id).await.unwrap().is_none());
        assert!(
            acl_member::list_members(&pool, a.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test]
    async fn manageable_includes_owned(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let other = accounts::create_account(&pool).await.unwrap();
        let mine = insert_acl_for_test(&pool, owner, "Mine").await;
        let _theirs = insert_acl_for_test(&pool, other, "Theirs").await;

        let acls = find_acls_manageable_by_account(&pool, owner).await.unwrap();
        assert_eq!(acls.len(), 1);
        assert_eq!(acls[0].id, mine.id);
    }

    #[sqlx::test]
    async fn manageable_includes_managed_via_character_member(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let manager = accounts::create_account(&pool).await.unwrap();
        let manager_char = insert_character(&pool, manager, 20, "Manager").await;

        let a = insert_acl_for_test(&pool, owner, "Managed").await;
        acl_member::add_member(
            &pool,
            a.id,
            "character",
            None,
            Some(manager_char),
            "Manager",
            "manage",
        )
        .await
        .unwrap();

        let acls = find_acls_manageable_by_account(&pool, manager)
            .await
            .unwrap();
        assert_eq!(acls.len(), 1);
        assert_eq!(acls[0].id, a.id);
    }

    #[sqlx::test]
    async fn manageable_excludes_read_only_member(pool: PgPool) {
        let owner = accounts::create_account(&pool).await.unwrap();
        let reader = accounts::create_account(&pool).await.unwrap();
        let reader_char = insert_character(&pool, reader, 30, "Reader").await;

        let a = insert_acl_for_test(&pool, owner, "ReadOnly").await;
        acl_member::add_member(
            &pool,
            a.id,
            "character",
            None,
            Some(reader_char),
            "Reader",
            "read",
        )
        .await
        .unwrap();

        // A plain `read` member does not make the ACL manageable.
        assert!(
            find_acls_manageable_by_account(&pool, reader)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
