pub mod app_state;
pub mod config;
pub mod db;
pub mod dto;
pub mod error;
pub mod esi;
pub mod handlers;
pub mod openapi;
pub mod response;
pub mod services;
pub mod session;

use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, post},
};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use app_state::AppState;
use handlers::middleware::refresh_session_cookie;

pub fn build_router(state: AppState) -> Router {
    let api_v1_routes = Router::new()
        .route("/me", get(handlers::api::v1::me::get_me))
        .route("/keys", post(handlers::api::v1::keys::create_key))
        .route("/keys", get(handlers::api::v1::keys::list_keys))
        .route("/keys/{id}", delete(handlers::api::v1::keys::delete_key))
        .route(
            "/characters/{id}/set-main",
            post(handlers::api::v1::characters::set_main),
        )
        .route(
            "/characters/{id}",
            delete(handlers::api::v1::characters::delete_character),
        )
        .route(
            "/account",
            delete(handlers::api::v1::account::delete_account),
        );

    Router::new()
        .route("/auth/login", get(handlers::auth::login))
        .route("/auth/callback", get(handlers::auth::callback))
        .route("/auth/logout", get(handlers::auth::logout))
        .route("/auth/characters/add", get(handlers::auth::add_character))
        .nest("/api/v1", api_v1_routes)
        // Public, unenveloped: the documented api-contract carve-out for /api/health.
        // Public by construction — get_health does not take the AuthenticatedAccount extractor.
        .route("/api/health", get(handlers::health::get_health))
        // SwaggerUi registers GET /api/openapi.json and GET /api/docs (+ /api/docs/*rest)
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", openapi::ApiDoc::openapi()))
        .layer(from_fn(refresh_session_cookie))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Returns all `/api/v1/*` routes as `(path, method)` pairs for doc-coverage tests.
pub fn registered_api_v1_routes() -> Vec<(String, String)> {
    vec![
        ("/api/v1/me".to_string(), "get".to_string()),
        ("/api/v1/keys".to_string(), "post".to_string()),
        ("/api/v1/keys".to_string(), "get".to_string()),
        ("/api/v1/keys/{id}".to_string(), "delete".to_string()),
        (
            "/api/v1/characters/{id}/set-main".to_string(),
            "post".to_string(),
        ),
        ("/api/v1/characters/{id}".to_string(), "delete".to_string()),
        ("/api/v1/account".to_string(), "delete".to_string()),
    ]
}
