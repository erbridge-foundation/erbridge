use std::sync::Arc;

use anyhow::Context;
use reqwest_middleware::ClientBuilder;
use reqwest_tracing::TracingMiddleware;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use backend::{
    app_state::AppState,
    config, db, esi,
    session::{InflightStore, SessionStore},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let config = config::Config::from_env().context("failed to load configuration")?;
    let config = Arc::new(config);

    let base_client = reqwest::Client::new();

    tracing::info!("fetching ESI discovery document");
    let esi_metadata = esi::discover(&base_client)
        .await
        .context("failed to discover ESI metadata")?;

    // RUST_LOG=erbridge=debug,reqwest_tracing=info to observe ESI call spans.
    let http_client = ClientBuilder::new(base_client)
        .with(TracingMiddleware::default())
        .build();
    let esi_metadata = Arc::new(esi_metadata);

    tracing::info!("connecting to database and running migrations");
    let db = db::connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    let session_store = SessionStore::new(db.clone());
    let inflight_store = InflightStore::new();

    let state = AppState {
        config,
        db,
        esi_metadata,
        session_store,
        inflight_store,
        http_client,
    };

    // Start the daily token-refresh sweep: detects character transfers
    // (owner-hash change) and expires stale / idle tokens. Cloned handles so the
    // task outlives `state`, which `build_router` consumes below.
    backend::services::token_sweep::spawn(
        state.db.clone(),
        state.http_client.clone(),
        state.esi_metadata.token_endpoint.clone(),
        state.config.esi_client_id.clone(),
        state.config.esi_client_secret.clone(),
        state.config.encryption_secret.clone(),
    );

    let app = backend::build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .context("failed to bind port 3000")?;

    tracing::info!("listening on port 3000");
    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
