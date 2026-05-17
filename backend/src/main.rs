use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use backend::{app_state::AppState, config, db, esi, session::SessionStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let config = config::Config::from_env().context("failed to load configuration")?;
    let config = Arc::new(config);

    let http_client = reqwest::Client::new();

    tracing::info!("fetching ESI discovery document");
    let esi_metadata = esi::discover(&http_client)
        .await
        .context("failed to discover ESI metadata")?;
    let esi_metadata = Arc::new(esi_metadata);

    tracing::info!("connecting to database and running migrations");
    let db = db::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let session_store = SessionStore::new();

    let state = AppState {
        config,
        db,
        esi_metadata,
        session_store,
        http_client,
    };

    let app = backend::build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .context("failed to bind port 3000")?;

    tracing::info!("listening on port 3000");
    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}
