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
    // The ESI rate-limit middleware sits after tracing so its waits/backoff are
    // traced; it keeps us within ESI's token-bucket and legacy error budgets.
    let esi_rate_limit = esi::rate_limit::EsiRateLimitMiddleware::new(
        config.rate_limit.esi_error_remain_threshold,
        config.rate_limit.esi_bucket_remain_threshold,
    );
    let http_client = ClientBuilder::new(base_client)
        .with(TracingMiddleware::default())
        .with(esi_rate_limit)
        .build();
    let esi_metadata = Arc::new(esi_metadata);

    // Fetch the SSO JWKS at startup (fail-fast like discovery — the app cannot
    // verify any identity without it). The cache refetches itself on rotation.
    tracing::info!("fetching ESI JWKS");
    let jwks = esi::jwks::JwksCache::fetch(http_client.clone(), &esi_metadata.jwks_uri)
        .await
        .context("failed to fetch ESI JWKS")?;
    let jwks = Arc::new(jwks);

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
        jwks,
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
        state.jwks.clone(),
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
    // Serve with connection info so the per-IP rate limiters can fall back to
    // the peer IP when the X-Forwarded-For header is absent.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .context("server error")?;

    Ok(())
}
