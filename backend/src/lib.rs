pub mod app_state;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod esi;
pub mod handlers;
pub mod response;
pub mod services;
pub mod session;

use axum::{routing::{delete, get, post}, Router};
use tower_http::trace::TraceLayer;

use app_state::AppState;

pub fn build_router(state: AppState) -> Router {
    let api_v1_routes = Router::new()
        .route("/keys", post(handlers::api::v1::keys::create_key))
        .route("/keys", get(handlers::api::v1::keys::list_keys))
        .route("/keys/{id}", delete(handlers::api::v1::keys::delete_key));

    Router::new()
        .route("/auth/login", get(handlers::auth::login))
        .route("/auth/callback", get(handlers::auth::callback))
        .route("/auth/logout", get(handlers::auth::logout))
        .route("/auth/characters/add", get(handlers::auth::add_character))
        .nest("/api/v1", api_v1_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
