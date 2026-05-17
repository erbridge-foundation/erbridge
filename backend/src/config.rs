use anyhow::{Context, Result};

pub struct Config {
    pub app_url: String,
    pub encryption_secret: String,
    pub esi_client_id: String,
    pub esi_client_secret: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            app_url: std::env::var("APP_URL")
                .context("APP_URL environment variable is required")?,
            encryption_secret: std::env::var("ENCRYPTION_SECRET")
                .context("ENCRYPTION_SECRET environment variable is required")?,
            esi_client_id: std::env::var("ESI_CLIENT_ID")
                .context("ESI_CLIENT_ID environment variable is required")?,
            esi_client_secret: std::env::var("ESI_CLIENT_SECRET")
                .context("ESI_CLIENT_SECRET environment variable is required")?,
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL environment variable is required")?,
        })
    }
}
