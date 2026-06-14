pub mod accounts;
pub mod acl;
pub mod acl_member;
pub mod api_keys;
pub mod blocks;
pub mod characters;
pub mod eve_system;
pub mod map;
pub mod map_acl;
pub mod preferences;
pub mod sessions;

#[cfg(test)]
pub mod test_helpers;

// No-new-twins rule: do not add byte-identical pool/tx pairs (`f` / `f_in_tx`).
// A function that may run against either a pool or a transaction takes
// `executor: impl PgExecutor<'_>` and the caller passes `&pool` or `&mut *tx`
// (see `api_keys::insert_key` / `delete_for_account`). Keep a tx-only variant
// (named `_in_tx`) ONLY when every caller is transactional AND the query is not
// trivially poolable — e.g. `accounts::count_server_admins_tx`, whose
// `FOR UPDATE` row lock is semantically distinct from the lock-free pool
// `count_server_admins` read; those two are NOT twins and both are kept.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("unique constraint violated: {constraint}")]
    UniqueViolation { constraint: String },

    #[error("check constraint violated: {constraint}")]
    CheckViolation { constraint: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err {
            if db_err.is_unique_violation() {
                let constraint = db_err.constraint().unwrap_or("<unknown>").to_string();
                return DbError::UniqueViolation { constraint };
            }
            // SQLSTATE 23514 = check_violation. Match on the code rather than the
            // message text so the mapping is locale- and wording-independent.
            if db_err.is_check_violation() {
                let constraint = db_err.constraint().unwrap_or("<unknown>").to_string();
                return DbError::CheckViolation { constraint };
            }
        }
        DbError::Other(anyhow::Error::from(err))
    }
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(30))
        .connect(database_url)
        .await
        .context("failed to connect to Postgres")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;

    Ok(pool)
}
