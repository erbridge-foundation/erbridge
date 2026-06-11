pub mod api_key;
pub mod app_state;
pub mod audit;
pub mod config;
pub mod crypto;
pub mod db;
pub mod dto;
pub mod error;
pub mod esi;
pub mod handlers;
pub mod openapi;
pub mod permissions;
pub mod response;
pub mod services;
pub mod session;

use axum::{
    Router,
    middleware::from_fn,
    routing::{delete, get, patch, post},
};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use app_state::AppState;
use handlers::middleware::refresh_session_cookie;

pub fn build_router(state: AppState) -> Router {
    let api_v1_routes = Router::new()
        .route("/me", get(handlers::api::v1::me::get_me))
        .route(
            "/me/preferences",
            get(handlers::api::v1::preferences::get_preferences),
        )
        .route(
            "/me/preferences",
            patch(handlers::api::v1::preferences::update_preferences),
        )
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
        )
        // ACLs
        .route("/acls", get(handlers::api::v1::acls::list_acls))
        .route("/acls", post(handlers::api::v1::acls::create_acl))
        .route("/acls/{acl_id}", patch(handlers::api::v1::acls::rename_acl))
        .route(
            "/acls/{acl_id}",
            delete(handlers::api::v1::acls::delete_acl),
        )
        .route(
            "/acls/{acl_id}/members",
            get(handlers::api::v1::acls::list_members),
        )
        .route(
            "/acls/{acl_id}/members",
            post(handlers::api::v1::acls::add_member),
        )
        .route(
            "/acls/{acl_id}/members/{member_id}",
            patch(handlers::api::v1::acls::update_member),
        )
        .route(
            "/acls/{acl_id}/members/{member_id}",
            delete(handlers::api::v1::acls::remove_member),
        )
        // Maps
        .route("/maps", get(handlers::api::v1::maps::list_maps))
        .route("/maps", post(handlers::api::v1::maps::create_map))
        .route("/maps/{map_id}", get(handlers::api::v1::maps::get_map))
        .route("/maps/{map_id}", patch(handlers::api::v1::maps::update_map))
        .route(
            "/maps/{map_id}",
            delete(handlers::api::v1::maps::delete_map),
        )
        .route(
            "/maps/{map_id}/acls",
            post(handlers::api::v1::maps::attach_acl),
        )
        .route(
            "/maps/{map_id}/acls/{acl_id}",
            delete(handlers::api::v1::maps::detach_acl),
        )
        // Entity search (account-authenticated; the ACL member picker builds on it)
        .route(
            "/entities/search",
            get(handlers::api::v1::entities::search_entities),
        );

    let admin_routes = Router::new()
        .route("/accounts", get(handlers::api::v1::admin::list_accounts))
        .route(
            "/characters/search",
            get(handlers::api::v1::admin::search_characters),
        )
        .route(
            "/characters/esi-search",
            get(handlers::api::v1::admin::esi_search_characters),
        )
        .route(
            "/accounts/{id}/grant-admin",
            post(handlers::api::v1::admin::grant_admin),
        )
        .route(
            "/accounts/{id}/revoke-admin",
            post(handlers::api::v1::admin::revoke_admin),
        )
        .route("/blocks", get(handlers::api::v1::admin::list_blocks))
        .route("/blocks", post(handlers::api::v1::admin::block_character))
        .route(
            "/blocks/{eve_character_id}",
            delete(handlers::api::v1::admin::unblock_character),
        )
        .route("/audit", get(handlers::api::v1::admin::list_audit));

    Router::new()
        .route("/auth/login", get(handlers::auth::login))
        .route("/auth/callback", get(handlers::auth::callback))
        .route("/auth/logout", get(handlers::auth::logout))
        .route("/auth/characters/add", get(handlers::auth::add_character))
        .nest("/api/v1", api_v1_routes)
        .nest("/api/v1/admin", admin_routes)
        // Public, unenveloped: the documented api-contract carve-out for /api/health.
        // Public by construction — get_health does not take the AuthenticatedAccount extractor.
        .route("/api/health", get(handlers::health::get_health))
        // SwaggerUi registers GET /api/openapi.json and GET /api/docs (+ /api/docs/*rest)
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", openapi::ApiDoc::openapi()))
        .layer(from_fn(refresh_session_cookie))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Returns all `/api/v1/admin/*` routes as `(path, method)` pairs for the
/// fail-closed admin-coverage test. Every entry here MUST be gated by the
/// `AdminAccount` extractor; the coverage test enforces that behaviourally by
/// asserting each route rejects an unauthenticated caller (401) and a
/// non-admin session (403). A handler that forgets the extractor would answer
/// differently and fail the test.
///
/// Kept in lockstep with the routes nested under `/api/v1/admin` in
/// `build_router`.
pub fn registered_admin_routes() -> Vec<(String, String)> {
    vec![
        ("/api/v1/admin/accounts".to_string(), "get".to_string()),
        (
            "/api/v1/admin/characters/search".to_string(),
            "get".to_string(),
        ),
        (
            "/api/v1/admin/characters/esi-search".to_string(),
            "get".to_string(),
        ),
        (
            "/api/v1/admin/accounts/{id}/grant-admin".to_string(),
            "post".to_string(),
        ),
        (
            "/api/v1/admin/accounts/{id}/revoke-admin".to_string(),
            "post".to_string(),
        ),
        ("/api/v1/admin/blocks".to_string(), "get".to_string()),
        ("/api/v1/admin/blocks".to_string(), "post".to_string()),
        (
            "/api/v1/admin/blocks/{eve_character_id}".to_string(),
            "delete".to_string(),
        ),
        ("/api/v1/admin/audit".to_string(), "get".to_string()),
    ]
}

/// Returns all `/api/v1/*` routes as `(path, method)` pairs for doc-coverage tests.
pub fn registered_api_v1_routes() -> Vec<(String, String)> {
    vec![
        ("/api/v1/me".to_string(), "get".to_string()),
        ("/api/v1/me/preferences".to_string(), "get".to_string()),
        ("/api/v1/me/preferences".to_string(), "patch".to_string()),
        ("/api/v1/keys".to_string(), "post".to_string()),
        ("/api/v1/keys".to_string(), "get".to_string()),
        ("/api/v1/keys/{id}".to_string(), "delete".to_string()),
        (
            "/api/v1/characters/{id}/set-main".to_string(),
            "post".to_string(),
        ),
        ("/api/v1/characters/{id}".to_string(), "delete".to_string()),
        ("/api/v1/account".to_string(), "delete".to_string()),
        ("/api/v1/acls".to_string(), "get".to_string()),
        ("/api/v1/acls".to_string(), "post".to_string()),
        ("/api/v1/acls/{acl_id}".to_string(), "patch".to_string()),
        ("/api/v1/acls/{acl_id}".to_string(), "delete".to_string()),
        (
            "/api/v1/acls/{acl_id}/members".to_string(),
            "get".to_string(),
        ),
        (
            "/api/v1/acls/{acl_id}/members".to_string(),
            "post".to_string(),
        ),
        (
            "/api/v1/acls/{acl_id}/members/{member_id}".to_string(),
            "patch".to_string(),
        ),
        (
            "/api/v1/acls/{acl_id}/members/{member_id}".to_string(),
            "delete".to_string(),
        ),
        ("/api/v1/maps".to_string(), "get".to_string()),
        ("/api/v1/maps".to_string(), "post".to_string()),
        ("/api/v1/maps/{map_id}".to_string(), "get".to_string()),
        ("/api/v1/maps/{map_id}".to_string(), "patch".to_string()),
        ("/api/v1/maps/{map_id}".to_string(), "delete".to_string()),
        ("/api/v1/maps/{map_id}/acls".to_string(), "post".to_string()),
        (
            "/api/v1/maps/{map_id}/acls/{acl_id}".to_string(),
            "delete".to_string(),
        ),
        ("/api/v1/entities/search".to_string(), "get".to_string()),
    ]
}
