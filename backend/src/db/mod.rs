pub mod accounts;
pub mod api_keys;
pub mod blocks;
pub mod characters;
pub mod preferences;
pub mod sessions;

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("unique constraint violated: {constraint}")]
    UniqueViolation { constraint: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(ref db_err) = err
            && db_err.is_unique_violation()
        {
            let constraint = db_err.constraint().unwrap_or("<unknown>").to_string();
            return DbError::UniqueViolation { constraint };
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
