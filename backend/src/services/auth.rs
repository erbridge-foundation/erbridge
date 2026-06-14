use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    audit::{self, ActingCharacter, AuditEvent, ServerAdminGrantSource},
    db::{accounts, accounts::ResolutionOutcome, blocks, characters},
    error::AppError,
};

/// The result of an SSO completion attempt. A blocked character is not an
/// error (nothing went wrong; the system worked as designed) — it is a distinct
/// outcome the handler maps to a `/blocked` redirect with no session, rather
/// than the authenticated happy path.
#[derive(Debug, PartialEq, Eq)]
pub enum SsoOutcome {
    /// SSO completed; the resolved account has a live session to establish.
    Authenticated(Uuid),
    /// The resolved character is in the block list. No account/character/token
    /// was written; a `blocked_login_rejected` audit row was recorded.
    Blocked,
    /// The add-character flow presented a character already bound to a
    /// *different* account. No write occurred to the existing row (no token
    /// overwrite, no `owner_hash`/public-info refresh); a
    /// `character_add_rejected_bound_elsewhere` audit row was recorded. The
    /// session is preserved — the conflict concerns the character, not the
    /// caller — so the handler keeps the existing session and redirects with a
    /// conflict flag.
    BoundElsewhere,
}

impl SsoOutcome {
    /// The authenticated account id, or `None` for a non-happy-path outcome.
    pub fn account_id(&self) -> Option<Uuid> {
        match self {
            SsoOutcome::Authenticated(id) => Some(*id),
            SsoOutcome::Blocked | SsoOutcome::BoundElsewhere => None,
        }
    }
}

/// Inputs to the post-ESI SSO completion path. The handler does the OAuth2
/// code exchange and ESI public-info fetches, then hands these values to this
/// service which owns the transactional DB writes and audit emissions.
pub struct SsoCompletionInput<'a> {
    pub add_character_account_id: Option<Uuid>,
    pub eve_character_id: i64,
    pub character_name: &'a str,
    pub corporation_id: i64,
    pub corporation_name: &'a str,
    pub alliance_id: Option<i64>,
    pub alliance_name: Option<&'a str>,
    pub esi_client_id: &'a str,
    pub access_token: &'a str,
    pub refresh_token: &'a str,
    pub access_token_expires_at: DateTime<Utc>,
    pub scopes: &'a [String],
    pub owner_hash: &'a str,
    pub encryption_key: &'a [u8],
}

/// Performs the SSO completion transaction: account resolution, character
/// upsert, main promotion, reactivation, and audit emissions. Returns the
/// resolved `account_id` (or [`SsoOutcome::Blocked`] when the character is
/// blocked).
pub async fn complete_sso_callback(
    pool: &PgPool,
    input: SsoCompletionInput<'_>,
) -> Result<SsoOutcome, AppError> {
    // Block check FIRST — before any account/character/token write, and before
    // `resolve_or_create` would create an account. Covers both the login flow
    // and the add-character flow (it precedes the resolve branch), so a blocked
    // pilot can neither sign in as themselves nor be attached as someone's alt.
    if blocks::is_eve_character_blocked(pool, input.eve_character_id).await? {
        // Record the rejected attempt in its own short transaction. actor is
        // NULL (no session); the subject character lives in `details`.
        let mut audit_tx = pool.begin().await.map_err(anyhow::Error::from)?;
        audit::record_in_tx(
            &mut audit_tx,
            None,
            None,
            AuditEvent::BlockedLoginRejected {
                eve_character_id: input.eve_character_id,
                character_name: Some(input.character_name.to_string()),
            },
        )
        .await?;
        audit_tx.commit().await.map_err(anyhow::Error::from)?;
        return Ok(SsoOutcome::Blocked);
    }

    let mut tx = pool.begin().await.map_err(anyhow::Error::from)?;

    // Transfer detection FIRST, inside the bind transaction. Look up the existing
    // row's (account_id, owner_hash) once, before any resolve/bind decision. A
    // character is *transferred* iff the presented owner hash is present, the
    // stored hash is non-null, and the two differ — CCP's canonical proof the
    // character changed EVE accounts. Anything else (absent presented hash, null
    // stored hash, matching hashes) is not a transfer and falls through to the
    // normal resolve/bind path below. A matching hash on the same account stays a
    // normal self-heal; the conservative path never detaches on unprovable
    // evidence.
    let existing_binding =
        characters::find_binding_and_owner_hash(&mut tx, input.eve_character_id).await?;
    if let Some((Some(former_account_id), Some(stored_hash))) = &existing_binding {
        let presented = input.owner_hash;
        let is_transfer = !presented.is_empty() && stored_hash.as_str() != presented;
        if is_transfer {
            // The destination differs only by mode: the session's account in
            // add-character mode (`Some`), or a freshly-minted account in login
            // mode (`None` → minted in `complete_transfer`). When the existing row
            // is *already* bound to the destination (re-auth of one's own
            // just-transferred-to-self character), there is nothing to detach —
            // fall through to the normal path.
            let destination = input.add_character_account_id;
            if destination != Some(*former_account_id) {
                return complete_transfer(tx, &input, *former_account_id, destination).await;
            }
        }
    }

    let (account_id, outcome) = accounts::resolve_or_create(
        &mut tx,
        input.add_character_account_id,
        input.eve_character_id,
    )
    .await?;

    // For add-character mode the pre-upsert binding is invisible to
    // `resolve_or_create` (it short-circuits on the session's account_id), so
    // look up the existing row here once, before any write. The single result
    // drives two decisions: refuse the add when the character is bound to a
    // *different* account, and distinguish an orphan claim from a fresh add for
    // the audit event. Other outcomes carry their answer in the enum variant.
    let mut add_character_is_orphan_claim = false;
    if matches!(outcome, ResolutionOutcome::AddCharacterMode) {
        let existing_binding =
            characters::find_account_id_for_eve_character(&mut tx, input.eve_character_id).await?;

        // Bound to a different account → conflict. Roll back the main tx (no
        // token overwrite, no public-info/owner_hash refresh on the other
        // account's row) and record the rejected attempt in its own short tx,
        // mirroring the blocked flow.
        if let Some(Some(other_account)) = existing_binding
            && other_account != account_id
        {
            tx.rollback().await.map_err(anyhow::Error::from)?;

            let mut audit_tx = pool.begin().await.map_err(anyhow::Error::from)?;
            audit::record_in_tx(
                &mut audit_tx,
                Some(account_id),
                None,
                AuditEvent::CharacterAddRejectedBoundElsewhere {
                    account_id,
                    eve_character_id: input.eve_character_id,
                },
            )
            .await?;
            audit_tx.commit().await.map_err(anyhow::Error::from)?;

            return Ok(SsoOutcome::BoundElsewhere);
        }

        // An orphan claim is an add-character flow over an existing row whose
        // `account_id` is NULL. (A bound-to-self row — re-adding one's own
        // character — is neither orphan nor conflict; it falls through to a
        // plain token refresh.)
        add_character_is_orphan_claim = matches!(existing_binding, Some(None));
    }

    let reactivated = accounts::reactivate_if_soft_deleted(&mut tx, account_id).await?;

    let character_id = characters::upsert_tokens(
        &mut tx,
        account_id,
        input.eve_character_id,
        input.character_name,
        input.corporation_id,
        input.corporation_name,
        input.alliance_id,
        input.alliance_name,
        input.esi_client_id,
        input.access_token,
        input.refresh_token,
        input.access_token_expires_at,
        input.scopes,
        input.owner_hash,
        input.encryption_key,
    )
    .await?;

    characters::promote_if_no_main(
        &mut tx,
        account_id,
        character_id,
        input.eve_character_id,
        input.character_name,
    )
    .await?;

    // Stamp the account-level login clock the daily sweep's idle waterfall reads.
    accounts::set_last_login(&mut tx, account_id).await?;

    // Audit emissions follow `promote_if_no_main` so any actor-account-id
    // emission resolves the main correctly. Login-time events (no session yet)
    // use the `acting_as` path; add-character mode (session present) uses
    // `actor_account_id`.
    let acting = ActingCharacter {
        eve_character_id: input.eve_character_id,
        name: input.character_name.to_string(),
    };
    match outcome {
        ResolutionOutcome::AddCharacterMode => {
            let event = if add_character_is_orphan_claim {
                AuditEvent::OrphanCharacterClaimed {
                    account_id,
                    eve_character_id: input.eve_character_id,
                    character_name: input.character_name.to_string(),
                }
            } else {
                AuditEvent::CharacterAdded {
                    account_id,
                    eve_character_id: input.eve_character_id,
                    character_name: input.character_name.to_string(),
                }
            };
            audit::record_in_tx(&mut tx, Some(account_id), None, event).await?;
        }
        ResolutionOutcome::NewAccount { bootstrapped_admin } => {
            audit::record_in_tx(
                &mut tx,
                None,
                Some(acting.clone()),
                AuditEvent::AccountRegistered {
                    account_id,
                    eve_character_id: input.eve_character_id,
                    character_name: input.character_name.to_string(),
                },
            )
            .await?;
            if bootstrapped_admin {
                audit::record_in_tx(
                    &mut tx,
                    None,
                    Some(acting.clone()),
                    AuditEvent::ServerAdminGranted {
                        account_id,
                        source: ServerAdminGrantSource::FirstAccountBootstrap,
                    },
                )
                .await?;
            }
        }
        ResolutionOutcome::OrphanCharacterExists => {
            audit::record_in_tx(
                &mut tx,
                None,
                Some(acting.clone()),
                AuditEvent::AccountRegistered {
                    account_id,
                    eve_character_id: input.eve_character_id,
                    character_name: input.character_name.to_string(),
                },
            )
            .await?;
            audit::record_in_tx(
                &mut tx,
                None,
                Some(acting.clone()),
                AuditEvent::OrphanCharacterClaimed {
                    account_id,
                    eve_character_id: input.eve_character_id,
                    character_name: input.character_name.to_string(),
                },
            )
            .await?;
        }
        ResolutionOutcome::ExistingAccount => {}
    }
    if reactivated {
        audit::record_in_tx(
            &mut tx,
            None,
            Some(acting),
            AuditEvent::AccountReactivated { account_id },
        )
        .await?;
    }

    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(SsoOutcome::Authenticated(account_id))
}

/// Completes a detected-transfer bind inside the already-open SSO-completion
/// `tx`. Mints a fresh destination account in login mode (`destination == None`)
/// or uses the session account in add-character mode (`destination == Some`),
/// detaches+rebinds the character to it, runs the seller-side fixup on the former
/// account (re-promote a remaining character or orphan it when emptied), emits a
/// `CharacterTransferred` audit event actored by the destination, reactivates the
/// destination if it was soft-deleted, stamps its `last_login`, and commits.
/// Returns `Authenticated(destination)`.
async fn complete_transfer(
    mut tx: Transaction<'_, Postgres>,
    input: &SsoCompletionInput<'_>,
    former_account_id: Uuid,
    destination: Option<Uuid>,
) -> Result<SsoOutcome, AppError> {
    // Resolve the destination account: the session's in add-character mode, or a
    // freshly-minted one in login mode (NOT `resolve_or_create`, which would
    // return the seller's account for this still-bound eve_character_id).
    let destination_account_id = match destination {
        Some(account_id) => account_id,
        None => accounts::create_fresh_account(&mut tx).await?.0,
    };

    // Snapshot the former account's display name BEFORE the seller-side fixup may
    // re-promote and overwrite its last_known_main_* (so the audit names the
    // account as it was at transfer time, fail-soft to None).
    let former_account_name =
        accounts::get_last_known_main_name(&mut tx, former_account_id).await?;

    // Detach the transferred row from the seller and rebind it to the destination,
    // overwriting tokens/owner-hash/scopes/public-info and stamping it valid. The
    // returned id is the rebound row (now is_main = FALSE on the destination).
    let rebound = characters::detach_and_rebind(
        &mut tx,
        destination_account_id,
        input.eve_character_id,
        input.character_name,
        input.corporation_id,
        input.corporation_name,
        input.alliance_id,
        input.alliance_name,
        input.esi_client_id,
        input.access_token,
        input.refresh_token,
        input.access_token_expires_at,
        input.scopes,
        input.owner_hash,
        input.encryption_key,
    )
    .await?;

    // Destination-side: if it had no main, the rebound character becomes it.
    characters::promote_if_no_main(
        &mut tx,
        destination_account_id,
        rebound.id,
        input.eve_character_id,
        input.character_name,
    )
    .await?;

    // Seller-side fixup: re-promote a remaining character if the seller lost its
    // main, or orphan the seller when it now has zero characters.
    let remaining = characters::count_for_account_in_tx(&mut tx, former_account_id).await?;
    if remaining == 0 {
        accounts::mark_orphaned(&mut tx, former_account_id).await?;
    } else {
        characters::promote_any_remaining_main(&mut tx, former_account_id).await?;
    }

    // A transferred character authenticating into a soft-deleted destination
    // reactivates it, mirroring the normal callback's self-heal.
    accounts::reactivate_if_soft_deleted(&mut tx, destination_account_id).await?;

    audit::record_in_tx(
        &mut tx,
        Some(destination_account_id),
        None,
        AuditEvent::CharacterTransferred {
            destination_account_id,
            eve_character_id: input.eve_character_id,
            character_name: input.character_name.to_string(),
            former_account_id,
            former_account_name,
        },
    )
    .await?;

    accounts::set_last_login(&mut tx, destination_account_id).await?;

    tx.commit().await.map_err(anyhow::Error::from)?;
    Ok(SsoOutcome::Authenticated(destination_account_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::characters as char_db;

    const KEY: &[u8] = &[0u8; 32];

    /// Seeds an account owning a main character (by `eve_id`) with `owner` hash.
    /// Returns the account id.
    async fn seller_with_main(pool: &PgPool, eve_id: i64, name: &str, owner: &str) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        let char_id = char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            name,
            1,
            "Corp",
            None,
            None,
            "client",
            "old-access",
            "old-refresh",
            Utc::now() + chrono::Duration::hours(1),
            &[],
            owner,
            KEY,
        )
        .await
        .unwrap();
        char_db::promote_if_no_main(&mut tx, account_id, char_id, eve_id, name)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    /// Adds a non-main character `eve_id` to `account_id`.
    async fn add_alt(pool: &PgPool, account_id: Uuid, eve_id: i64, owner: &str) {
        let mut tx = pool.begin().await.unwrap();
        char_db::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            "Alt",
            1,
            "Corp",
            None,
            None,
            "client",
            "a",
            "r",
            Utc::now() + chrono::Duration::hours(1),
            &[],
            owner,
            KEY,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    /// Builds an `SsoCompletionInput` presenting `owner` for `eve_id`.
    fn input<'a>(
        add_character_account_id: Option<Uuid>,
        eve_id: i64,
        name: &'a str,
        owner: &'a str,
    ) -> SsoCompletionInput<'a> {
        SsoCompletionInput {
            add_character_account_id,
            eve_character_id: eve_id,
            character_name: name,
            corporation_id: 1,
            corporation_name: "Corp",
            alliance_id: None,
            alliance_name: None,
            esi_client_id: "client",
            access_token: "new-access",
            refresh_token: "new-refresh",
            access_token_expires_at: Utc::now() + chrono::Duration::hours(1),
            scopes: &[],
            owner_hash: owner,
            encryption_key: KEY,
        }
    }

    async fn account_id_of(pool: &PgPool, eve_id: i64) -> Option<Uuid> {
        sqlx::query!(
            "SELECT account_id FROM eve_character WHERE eve_character_id = $1",
            eve_id
        )
        .fetch_one(pool)
        .await
        .unwrap()
        .account_id
    }

    #[sqlx::test]
    async fn login_transfer_lands_in_fresh_account_not_seller(pool: PgPool) {
        let seller = seller_with_main(&pool, 1000, "Sold", "owner-old").await;

        // The buyer logs in fresh; the presented hash differs from the stored one.
        let outcome = complete_sso_callback(&pool, input(None, 1000, "Sold", "owner-new"))
            .await
            .unwrap();
        let dest = match outcome {
            SsoOutcome::Authenticated(id) => id,
            other => panic!("expected Authenticated, got {other:?}"),
        };
        assert_ne!(dest, seller, "must NOT land in the seller's account");
        assert_eq!(account_id_of(&pool, 1000).await, Some(dest));

        // The seller is emptied → orphaned, its snapshot retained.
        let seller_acc = accounts::get_account(&pool, seller).await.unwrap().unwrap();
        assert_eq!(seller_acc.status, "orphaned");

        // The transfer is audited.
        let n = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM audit_log WHERE event_type = 'character_transferred'"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(n, 1);
    }

    #[sqlx::test]
    async fn add_character_transfer_rebinds_to_session_account(pool: PgPool) {
        let seller = seller_with_main(&pool, 2000, "Sold", "owner-old").await;
        let buyer = seller_with_main(&pool, 2001, "Buyer Main", "buyer-hash").await;

        // Buyer's session adds the transferred character (differing hash) → rebind,
        // never bound-elsewhere.
        let outcome = complete_sso_callback(&pool, input(Some(buyer), 2000, "Sold", "owner-new"))
            .await
            .unwrap();
        assert_eq!(outcome, SsoOutcome::Authenticated(buyer));
        assert_eq!(account_id_of(&pool, 2000).await, Some(buyer));

        // Seller emptied → orphaned.
        let seller_acc = accounts::get_account(&pool, seller).await.unwrap().unwrap();
        assert_eq!(seller_acc.status, "orphaned");
    }

    #[sqlx::test]
    async fn transfer_repromotes_seller_main_when_alt_remains(pool: PgPool) {
        let seller = seller_with_main(&pool, 3000, "Sold Main", "owner-old").await;
        add_alt(&pool, seller, 3001, "owner-old").await;

        complete_sso_callback(&pool, input(None, 3000, "Sold Main", "owner-new"))
            .await
            .unwrap();

        // Seller kept the alt, was re-promoted, not orphaned.
        let seller_acc = accounts::get_account(&pool, seller).await.unwrap().unwrap();
        assert_eq!(seller_acc.status, "active");
        let chars = char_db::list_for_account(&pool, seller).await.unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].eve_character_id, 3001);
        assert!(chars[0].is_main);
    }

    #[sqlx::test]
    async fn matching_hash_login_is_normal_self_heal_no_transfer(pool: PgPool) {
        let seller = seller_with_main(&pool, 4000, "Mine", "same-hash").await;

        // Same hash on the same account → normal re-login, no detach/transfer.
        let outcome = complete_sso_callback(&pool, input(None, 4000, "Mine", "same-hash"))
            .await
            .unwrap();
        assert_eq!(outcome, SsoOutcome::Authenticated(seller));
        assert_eq!(account_id_of(&pool, 4000).await, Some(seller));

        let n = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM audit_log WHERE event_type = 'character_transferred'"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
        .c;
        assert_eq!(n, 0, "matching hash is not a transfer");
    }

    #[sqlx::test]
    async fn null_stored_hash_add_character_is_not_a_transfer(pool: PgPool) {
        // A seller character with NULL stored owner_hash (legacy / never observed).
        let seller = accounts::create_account(&pool).await.unwrap();
        char_db::create_orphan(&pool, 5000, "Legacy", 1, "Corp", None, None)
            .await
            .unwrap();
        sqlx::query!(
            "UPDATE eve_character SET account_id = $1, owner_hash = NULL WHERE eve_character_id = 5000",
            seller
        )
        .execute(&pool)
        .await
        .unwrap();

        let buyer = seller_with_main(&pool, 5001, "Buyer", "buyer-hash").await;

        // Null stored hash → not a transfer → the existing bound-elsewhere
        // rejection applies (the character is bound to the seller, not the buyer).
        let outcome = complete_sso_callback(&pool, input(Some(buyer), 5000, "Legacy", "presented"))
            .await
            .unwrap();
        assert_eq!(outcome, SsoOutcome::BoundElsewhere);
        assert_eq!(account_id_of(&pool, 5000).await, Some(seller), "unchanged");
    }
}
