use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    api_key,
    audit::{self, AuditEvent},
    db::{DbError, api_keys as db},
    error::{AppError, ConflictKind},
};

#[derive(Debug)]
pub struct CreatedKey {
    pub id: Uuid,
    pub plaintext: String,
    pub name: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Generates a new API key, hashes it, and inserts a row with `scope = 'account'`.
/// The plaintext key is returned only here and never stored.
pub async fn create_key(
    pool: &PgPool,
    account_id: Uuid,
    name: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<CreatedKey, AppError> {
    let plaintext = api_key::generate();
    let key_hash = api_key::hash(&plaintext);

    let mut tx = pool.begin().await?;

    let (id, created_at) = db::insert_key(&mut *tx, account_id, name, &key_hash, expires_at)
        .await
        .map_err(|e| match e {
            DbError::UniqueViolation { .. } => {
                AppError::Conflict(ConflictKind::ApiKeyNameAlreadyExists)
            }
            DbError::CheckViolation { constraint } => {
                AppError::Internal(anyhow::anyhow!("unexpected check violation: {constraint}"))
            }
            DbError::Other(err) => AppError::Internal(err),
        })?;

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::ApiKeyCreated {
            account_id,
            key_id: id,
            name: name.to_string(),
        },
    )
    .await?;

    tx.commit().await?;

    Ok(CreatedKey {
        id,
        plaintext,
        name: name.to_string(),
        expires_at,
        created_at,
    })
}

/// Resolves an API key (by plaintext) to the bearer-auth row carrying the key's
/// scope plus the owning account's status and block state — everything the
/// extractor needs to authorise in a single query. Hashes internally before
/// querying. Returns `None` if no live (unexpired) key matches.
pub async fn lookup_by_plaintext(
    pool: &PgPool,
    plaintext: &str,
) -> Result<Option<db::BearerKeyRow>, AppError> {
    let key_hash = api_key::hash(plaintext);
    Ok(db::find_by_hash(pool, &key_hash).await?)
}

pub async fn list_keys(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Vec<db::ApiKeyMetadata>, AppError> {
    Ok(db::list_for_account(pool, account_id).await?)
}

pub async fn delete_key(pool: &PgPool, id: Uuid, account_id: Uuid) -> Result<bool, AppError> {
    let mut tx = pool.begin().await?;

    let key_name = match db::delete_for_account(&mut *tx, id, account_id).await? {
        Some(name) => name,
        None => {
            // Nothing to audit — roll back the empty tx and return.
            tx.rollback().await?;
            return Ok(false);
        }
    };

    audit::record_in_tx(
        &mut tx,
        Some(account_id),
        None,
        AuditEvent::ApiKeyRevoked {
            account_id,
            key_id: id,
            key_name,
        },
    )
    .await?;

    tx.commit().await?;

    Ok(true)
}

/// Validates that a name is non-empty after trimming.
pub fn validate_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Err(AppError::BadRequest("name is required".to_string()))
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_accepts_non_empty() {
        assert_eq!(validate_name("my-key").unwrap(), "my-key");
        assert_eq!(validate_name("  trimmed  ").unwrap(), "trimmed");
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(matches!(validate_name(""), Err(AppError::BadRequest(_))));
        assert!(matches!(validate_name("   "), Err(AppError::BadRequest(_))));
    }
}
