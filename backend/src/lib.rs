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

use std::sync::Arc;

use axum::{
    Router,
    middleware::from_fn,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post},
};
use tower_governor::{
    GovernorLayer,
    errors::GovernorError,
    governor::GovernorConfigBuilder,
    key_extractor::{KeyExtractor, SmartIpKeyExtractor},
};
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use app_state::AppState;
use handlers::middleware::refresh_session_cookie;

/// Upper bound on how long any single request may take before the server aborts
/// it with a 408. No streaming endpoints exist yet; revisit (exclude SSE) when
/// they land. Tunable here.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Path the auth rate limiter redirects to when an `/auth/*` request is
/// throttled. The api-contract exempts `/auth/*` from the JSON envelope, so a
/// browser-facing redirect is used instead of `rate_limited`. The frontend
/// route that renders this page is a follow-up.
pub const AUTH_TOO_BUSY_PATH: &str = "/too-busy";

/// Per-client-IP key extractor for the inbound limiters.
///
/// Wraps `SmartIpKeyExtractor` (which reads the `X-Forwarded-For` / `Forwarded`
/// headers Traefik sets, falling back to the peer IP) and substitutes a fixed
/// sentinel key when no IP can be determined at all. Behind Traefik, a real
/// client IP is always present so the sentinel is never used in production; it
/// exists so a request with no IP signal (e.g. an in-process test `oneshot`,
/// which carries no connection info) is keyed deterministically rather than
/// rejected outright by the limiter.
#[derive(Clone)]
struct ClientIpKeyExtractor;

impl KeyExtractor for ClientIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        SmartIpKeyExtractor
            .extract(req)
            .or(Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
    }
}

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
        .route("/acls/{acl_id}", get(handlers::api::v1::acls::get_acl))
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
        .route(
            "/maps/by-slug/{slug}",
            get(handlers::api::v1::maps::get_map_by_slug),
        )
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

    let rl = state.config.rate_limit.clone();

    // Builds a per-IP governor config. Period and burst are clamped to a
    // non-zero minimum so `finish()` is always `Some` (it only returns `None`
    // for a zero period or burst), keeping router construction infallible even
    // if misconfigured. `ClientIpKeyExtractor` reads the X-Forwarded-For Traefik
    // sets, falling back to the peer IP.
    let governor_config = |per_millis: u64, burst: u32| {
        let mut builder = GovernorConfigBuilder::default().key_extractor(ClientIpKeyExtractor);
        builder
            .per_millisecond(per_millis.max(1))
            .burst_size(burst.max(1))
            .finish()
    };

    // Inbound per-IP limiter for /api/* — rejects with the standard 429
    // `rate_limited` envelope + Retry-After.
    let api_governor = governor_config(rl.api_per_millis, rl.api_burst);

    // Inbound per-IP limiter for /auth/* — tighter, and on reject it redirects
    // to the too-busy page rather than emitting the JSON envelope.
    let auth_governor = governor_config(rl.auth_per_millis, rl.auth_burst);

    // The clamps above guarantee both configs are `Some`. If a config somehow
    // failed to build we skip its layer rather than panic — the routes stay up,
    // just un-throttled — which a warning surfaces.
    let api_routes = Router::new()
        .nest("/api/v1", api_v1_routes)
        .nest("/api/v1/admin", admin_routes);
    let api_routes = match api_governor {
        Some(cfg) => api_routes.layer(
            GovernorLayer::new(Arc::new(cfg)).error_handler(|err| api_rate_limit_response(&err)),
        ),
        None => {
            tracing::warn!("/api rate-limit config invalid; /api/* is un-throttled");
            api_routes
        }
    };

    let auth_routes = Router::new()
        .route("/auth/login", get(handlers::auth::login))
        .route("/auth/callback", get(handlers::auth::callback))
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/characters/add", get(handlers::auth::add_character));
    let auth_routes = match auth_governor {
        Some(cfg) => auth_routes
            .layer(GovernorLayer::new(Arc::new(cfg)).error_handler(|_| auth_rate_limit_response())),
        None => {
            tracing::warn!("/auth rate-limit config invalid; /auth/* is un-throttled");
            auth_routes
        }
    };

    Router::new()
        .merge(auth_routes)
        .merge(api_routes)
        // Public, unenveloped: the documented api-contract carve-out for /api/health.
        // Public by construction — get_health does not take the AuthenticatedAccount extractor.
        // Intentionally outside the /api/* limiter so liveness probes are never throttled.
        .route("/api/health", get(handlers::health::get_health))
        // SwaggerUi registers GET /api/openapi.json and GET /api/docs (+ /api/docs/*rest)
        .merge(SwaggerUi::new("/api/docs").url("/api/openapi.json", openapi::ApiDoc::openapi()))
        .layer(from_fn(refresh_session_cookie))
        // Abort any request that runs longer than the timeout. Outermost of the
        // app layers so the whole handler stack (including cookie refresh) is
        // bounded; the limiter layers above remain unaffected.
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Maps a governor rejection on `/api/*` to the canonical `rate_limited` 429
/// envelope with a `Retry-After` header.
fn api_rate_limit_response(err: &tower_governor::GovernorError) -> Response {
    let retry_after = match err {
        tower_governor::GovernorError::TooManyRequests { wait_time, .. } => *wait_time,
        _ => 1,
    };
    error::rate_limited_response(retry_after)
}

/// Maps a governor rejection on `/auth/*` to a redirect to the too-busy page.
/// Never emits the `rate_limited` JSON envelope (api-contract exempts /auth/*).
fn auth_rate_limit_response() -> Response {
    Redirect::to(AUTH_TOO_BUSY_PATH).into_response()
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
        ("/api/v1/acls/{acl_id}".to_string(), "get".to_string()),
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
        ("/api/v1/maps/by-slug/{slug}".to_string(), "get".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    /// The request-timeout layer aborts a handler that outlives `REQUEST_TIMEOUT`.
    /// Exercised here with the same `TimeoutLayer` the real router uses but a
    /// tiny duration and a deliberately slow route, so the test is fast.
    #[tokio::test]
    async fn timeout_layer_aborts_slow_request() {
        let app = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    "never"
                }),
            )
            .layer(TimeoutLayer::with_status_code(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                std::time::Duration::from_millis(50),
            ));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/slow")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("service responds");

        // TimeoutLayer surfaces an elapsed request as 408 Request Timeout.
        assert_eq!(resp.status(), axum::http::StatusCode::REQUEST_TIMEOUT);
    }

    /// A fast handler under the same layer is unaffected.
    #[tokio::test]
    async fn timeout_layer_passes_fast_request() {
        let app = Router::new().route("/fast", get(|| async { "ok" })).layer(
            TimeoutLayer::with_status_code(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                std::time::Duration::from_secs(30),
            ),
        );

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/fast")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("service responds");

        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }
}
