mod app_state;
mod config;
mod db;
mod error;
mod esi;
mod handlers;
mod response;
mod session;

use std::sync::Arc;

use anyhow::Context;
use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use app_state::AppState;
use session::SessionStore;

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

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .context("failed to bind port 3000")?;

    tracing::info!("listening on port 3000");
    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/auth/login", get(handlers::auth::login))
        .route("/auth/callback", get(handlers::auth::callback))
        .route("/auth/logout", get(handlers::auth::logout))
        .route("/auth/characters/add", get(handlers::auth::add_character))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
