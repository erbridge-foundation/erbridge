//! Daily token-refresh sweep.
//!
//! Once a day the sweep refreshes every character's stored EVE token. The
//! refreshed access-token JWT re-exposes the `owner` claim, which CCP rotates on
//! a character transfer; comparing it against the stored hash is the only
//! reliable transfer signal (CCP does not revoke refresh tokens on transfer).
//!
//! Per character the sweep applies one of three outcomes, then runs a 7-day
//! account-idle waterfall. See the `character-token-lifecycle` capability.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::audit::{self, AuditEvent};
use crate::db::{accounts, characters};
use crate::esi::token;
use crate::handlers::crypto;

/// How long an account may go without logging in before the waterfall expires
/// its still-valid character tokens.
const IDLE_DAYS: i64 = 7;

/// The action a single character's refresh result implies. Pure decision,
/// separated from I/O so it can be unit-tested exhaustively.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SweepAction {
    /// Refresh succeeded and the owner hash matched (or no prior hash to compare
    /// against): store the rotated tokens and keep the character valid.
    KeepValid,
    /// Refresh succeeded but the owner hash differs from a non-null stored hash:
    /// the character was transferred. Flag `owner_mismatch` and audit.
    OwnerMismatch,
    /// Refresh failed: flag `token_expired`.
    Expired,
}

/// Decides the outcome for one character from its stored owner hash and the
/// refresh result (`Some(refreshed_owner_hash)` on success, `None` on failure).
pub(crate) fn decide(
    stored_owner_hash: Option<&str>,
    refreshed_owner_hash: Option<&str>,
) -> SweepAction {
    match refreshed_owner_hash {
        None => SweepAction::Expired,
        Some(new_hash) => match stored_owner_hash {
            // A non-null stored hash that differs is proof of transfer.
            Some(old) if old != new_hash => SweepAction::OwnerMismatch,
            // Equal, or no prior hash to compare against → healthy.
            _ => SweepAction::KeepValid,
        },
    }
}

/// Spawns the sweep on a ~24h interval. The first tick fires immediately at
/// startup. A single run's failure is logged and never kills the loop.
pub fn spawn(
    pool: PgPool,
    http: reqwest_middleware::ClientWithMiddleware,
    token_endpoint: String,
    client_id: String,
    client_secret: String,
    encryption_secret: String,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(24 * 60 * 60));
        loop {
            ticker.tick().await;
            if let Err(e) = run_once(
                &pool,
                &http,
                &token_endpoint,
                &client_id,
                &client_secret,
                &encryption_secret,
            )
            .await
            {
                error!("token-refresh sweep run failed: {e:#}");
            }
        }
    });
}

/// One full sweep pass: refresh-and-classify every eligible character, then the
/// idle waterfall. Returns an error only for failures that abort the whole run
/// (e.g. the initial listing query); per-character refresh failures are normal
/// and handled inline.
pub async fn run_once(
    pool: &PgPool,
    http: &reqwest_middleware::ClientWithMiddleware,
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    encryption_secret: &str,
) -> anyhow::Result<()> {
    let encryption_key = crypto::token_encryption_key(encryption_secret)?;

    let eligible = characters::list_refreshable(pool).await?;
    let total = eligible.len();
    let (mut kept, mut mismatched, mut expired) = (0u32, 0u32, 0u32);

    for ch in eligible {
        // Decrypt the stored refresh token; an undecryptable token is treated as
        // a refresh failure (we cannot use it).
        let refreshed = match crypto::decrypt_token(&ch.encrypted_refresh_token, &encryption_key) {
            Ok(refresh_plaintext) => {
                token::refresh_access_token(
                    http,
                    token_endpoint,
                    client_id,
                    client_secret,
                    &refresh_plaintext,
                )
                .await
            }
            Err(_) => None,
        };

        let refreshed_hash = refreshed.as_ref().map(|r| r.owner_hash.as_str());
        match decide(ch.owner_hash.as_deref(), refreshed_hash) {
            SweepAction::KeepValid => {
                // Safe to unwrap the option: KeepValid only arises from Some(_).
                if let Some(r) = refreshed {
                    if let Err(e) = characters::update_tokens_by_eve_id(
                        pool,
                        ch.eve_character_id,
                        &r.access_token,
                        &r.refresh_token,
                        r.access_token_expires_at,
                        &r.owner_hash,
                        &encryption_key,
                    )
                    .await
                    {
                        warn!(
                            eve_character_id = ch.eve_character_id,
                            "failed to persist refreshed tokens: {e:#}"
                        );
                    }
                    kept += 1;
                }
            }
            SweepAction::OwnerMismatch => {
                let new_hash = refreshed.as_ref().map(|r| r.owner_hash.as_str());
                if let Err(e) = characters::set_token_status(
                    pool,
                    ch.eve_character_id,
                    "owner_mismatch",
                    new_hash,
                )
                .await
                {
                    warn!(
                        eve_character_id = ch.eve_character_id,
                        "failed to flag owner_mismatch: {e:#}"
                    );
                    continue;
                }
                // Audit the transfer (no session → actor NULL; the previous owner
                // and subject character live in the event payload).
                if let Some(account_id) = ch.account_id {
                    let mut tx = pool.begin().await?;
                    audit::record_in_tx(
                        &mut tx,
                        None,
                        None,
                        AuditEvent::CharacterOwnerMismatch {
                            account_id,
                            eve_character_id: ch.eve_character_id,
                        },
                    )
                    .await?;
                    tx.commit().await?;
                }
                mismatched += 1;
            }
            SweepAction::Expired => {
                if let Err(e) =
                    characters::set_token_status(pool, ch.eve_character_id, "token_expired", None)
                        .await
                {
                    warn!(
                        eve_character_id = ch.eve_character_id,
                        "failed to flag token_expired: {e:#}"
                    );
                    continue;
                }
                expired += 1;
            }
        }
    }

    // 7-day idle waterfall: expire still-valid characters of idle accounts.
    let mut idle_expired = 0u64;
    for account_id in accounts::list_idle_accounts(pool, IDLE_DAYS).await? {
        match characters::expire_valid_tokens_for_account(pool, account_id).await {
            Ok(n) => idle_expired += n,
            Err(e) => warn!(%account_id, "failed to expire idle account tokens: {e:#}"),
        }
    }

    info!(
        total,
        kept, mismatched, expired, idle_expired, "token-refresh sweep complete"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_failure_is_expired() {
        assert_eq!(decide(Some("h"), None), SweepAction::Expired);
        assert_eq!(decide(None, None), SweepAction::Expired);
    }

    #[test]
    fn matching_hash_keeps_valid() {
        assert_eq!(decide(Some("h"), Some("h")), SweepAction::KeepValid);
    }

    #[test]
    fn differing_hash_is_owner_mismatch() {
        assert_eq!(decide(Some("old"), Some("new")), SweepAction::OwnerMismatch);
    }

    #[test]
    fn null_stored_hash_records_without_mismatch() {
        // A character first observed (no stored hash) records the new hash and
        // stays valid — never a false transfer.
        assert_eq!(decide(None, Some("new")), SweepAction::KeepValid);
    }
}

#[cfg(test)]
mod run_once_tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use reqwest_middleware::ClientBuilder;
    use sqlx::PgPool;
    use uuid::Uuid;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// 64 hex zeros → a 32-byte all-zero key, matching the seed encryption.
    const SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    const KEY: &[u8] = &[0u8; 32];

    fn http() -> reqwest_middleware::ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new()).build()
    }

    /// A JWT-shaped access token carrying the given owner hash.
    fn access_jwt(owner: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            format!(r#"{{"sub":"CHARACTER:EVE:1","name":"P","owner":"{owner}","scp":"a"}}"#)
                .as_bytes(),
        );
        format!("{header}.{payload}.sig")
    }

    async fn seed(pool: &PgPool, eve_id: i64, owner: &str) -> Uuid {
        let account_id = accounts::create_account(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        characters::upsert_tokens(
            &mut tx,
            account_id,
            eve_id,
            "Pilot",
            1,
            "Corp",
            None,
            None,
            "c",
            "access",
            "refresh",
            chrono::Utc::now() + chrono::Duration::hours(1),
            &["a".to_string()],
            owner,
            KEY,
        )
        .await
        .unwrap();
        // Stamp last_login so the idle waterfall does not interfere.
        accounts::set_last_login(&mut tx, account_id).await.unwrap();
        tx.commit().await.unwrap();
        account_id
    }

    async fn status_of(pool: &PgPool, eve_id: i64) -> String {
        sqlx::query!(
            "SELECT token_status FROM eve_character WHERE eve_character_id = $1",
            eve_id
        )
        .fetch_one(pool)
        .await
        .unwrap()
        .token_status
    }

    /// A mock token endpoint returning a refresh response whose access token
    /// carries `owner`.
    async fn mock_refresh_returning(owner: &str) -> (MockServer, String) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": access_jwt(owner),
                "refresh_token": "rotated-refresh",
                "expires_in": 1200,
            })))
            .mount(&server)
            .await;
        let endpoint = format!("{}/oauth/token", server.uri());
        (server, endpoint)
    }

    #[sqlx::test]
    async fn matching_hash_keeps_valid_and_rotates(pool: PgPool) {
        seed(&pool, 1, "hash-x").await;
        let (_s, endpoint) = mock_refresh_returning("hash-x").await;

        run_once(&pool, &http(), &endpoint, "id", "secret", SECRET)
            .await
            .unwrap();

        assert_eq!(status_of(&pool, 1).await, "valid");
    }

    #[sqlx::test]
    async fn changed_hash_flags_owner_mismatch_and_audits(pool: PgPool) {
        let account_id = seed(&pool, 1, "hash-old").await;
        let (_s, endpoint) = mock_refresh_returning("hash-new").await;

        run_once(&pool, &http(), &endpoint, "id", "secret", SECRET)
            .await
            .unwrap();

        assert_eq!(status_of(&pool, 1).await, "owner_mismatch");
        // Credentials wiped, new hash recorded.
        let r = sqlx::query!(
            "SELECT owner_hash, encrypted_refresh_token FROM eve_character WHERE eve_character_id = 1"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(r.owner_hash.as_deref(), Some("hash-new"));
        assert!(r.encrypted_refresh_token.is_none());
        // An audit row was written for the transfer.
        let audited = sqlx::query!(
            "SELECT COUNT(*) AS \"c!\" FROM audit_log
             WHERE event_type = 'character_owner_mismatch' AND details->>'account_id' = $1",
            account_id.to_string()
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(audited.c, 1);
    }

    #[sqlx::test]
    async fn refresh_failure_flags_token_expired(pool: PgPool) {
        seed(&pool, 1, "hash-x").await;
        // Token endpoint rejects the refresh.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;
        let endpoint = format!("{}/oauth/token", server.uri());

        run_once(&pool, &http(), &endpoint, "id", "secret", SECRET)
            .await
            .unwrap();

        assert_eq!(status_of(&pool, 1).await, "token_expired");
    }

    #[sqlx::test]
    async fn idle_account_tokens_are_expired(pool: PgPool) {
        let account_id = seed(&pool, 1, "hash-x").await;
        // Force the account well past the idle threshold.
        sqlx::query!(
            "UPDATE account SET last_login = now() - interval '30 days' WHERE id = $1",
            account_id
        )
        .execute(&pool)
        .await
        .unwrap();
        // Refresh would otherwise succeed and keep it valid...
        let (_s, endpoint) = mock_refresh_returning("hash-x").await;

        run_once(&pool, &http(), &endpoint, "id", "secret", SECRET)
            .await
            .unwrap();

        // ...but the idle waterfall expires it in the same run.
        assert_eq!(status_of(&pool, 1).await, "token_expired");
    }
}
