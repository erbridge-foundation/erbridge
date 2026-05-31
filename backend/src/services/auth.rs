use chrono::{DateTime, Utc};
use sqlx::PgPool;
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
}

impl SsoOutcome {
    /// The authenticated account id, or `None` for a blocked outcome.
    pub fn account_id(&self) -> Option<Uuid> {
        match self {
            SsoOutcome::Authenticated(id) => Some(*id),
            SsoOutcome::Blocked => None,
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
            },
        )
        .await?;
        audit_tx.commit().await.map_err(anyhow::Error::from)?;
        return Ok(SsoOutcome::Blocked);
    }

    let mut tx = pool.begin().await.map_err(anyhow::Error::from)?;

    let (account_id, outcome) = accounts::resolve_or_create(
        &mut tx,
        input.add_character_account_id,
        input.eve_character_id,
    )
    .await?;

    // For add-character mode the orphan-vs-fresh distinction is invisible to
    // `resolve_or_create` (it short-circuits on the session's account_id), so
    // look up the pre-upsert state here. Other outcomes carry the answer in
    // the enum variant.
    let add_character_is_orphan_claim = matches!(outcome, ResolutionOutcome::AddCharacterMode)
        && characters::find_account_id_for_eve_character(&mut tx, input.eve_character_id)
            .await?
            .map(|account| account.is_none())
            .unwrap_or(false);

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
        input.encryption_key,
    )
    .await?;

    characters::promote_if_no_main(&mut tx, account_id, character_id).await?;

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
