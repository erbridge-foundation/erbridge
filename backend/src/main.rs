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
    backend::services::token_sweep::spawn(backend::services::token_sweep::SweepContext {
        pool: state.db.clone(),
        http: state.http_client.clone(),
        jwks: state.jwks.clone(),
        token_endpoint: state.esi_metadata.token_endpoint.clone(),
        client_id: state.config.esi_client_id.clone(),
        client_secret: state.config.esi_client_secret.clone(),
        encryption_secret: state.config.encryption_secret.clone(),
    });

    // Start the daily EVE system-catalog sync: refreshes the system spine,
    // wormhole-type dictionary, and per-system statics from eve-scout + anoikis.
    // Cloned handles so the task outlives `state`.
    backend::services::eve_system_sync::spawn(backend::services::eve_system_sync::SyncContext {
        pool: state.db.clone(),
        http: state.http_client.clone(),
        systems_url: state.config.catalog.systems_url.clone(),
        wormhole_types_url: state.config.catalog.wormhole_types_url.clone(),
        statics_url: state.config.catalog.statics_url.clone(),
        user_agent: state.config.catalog.user_agent.clone(),
    });

    let bind_addr = state.config.bind_addr.clone();
    let app = backend::build_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;

    tracing::info!("listening on {bind_addr}");
    // Serve with connection info so the per-IP rate limiters can fall back to
    // the peer IP when the X-Forwarded-For header is absent. Graceful shutdown
    // on SIGTERM/ctrl-c lets in-flight requests drain on deploy restarts.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")?;

    Ok(())
}

/// Resolves when the process receives SIGTERM (orchestrator stop) or ctrl-c
/// (SIGINT, interactive). Either triggers a graceful drain of in-flight requests.
async fn shutdown_signal() {
    let ctrl_c = async {
        // Installing the ctrl-c handler fails only on a fundamentally broken OS
        // signal setup at startup; there is no sensible recovery, and a panic
        // here aborts boot rather than running without a shutdown signal.
        #[allow(clippy::expect_used)]
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl-c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        // Same rationale as ctrl-c: a SIGTERM handler that cannot be installed
        // is an unrecoverable startup fault, so panicking at boot is correct.
        #[allow(clippy::expect_used)]
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received ctrl-c, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
