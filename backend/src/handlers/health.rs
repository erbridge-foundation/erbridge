use axum::{Json, extract::State};

use crate::{
    app_state::AppState,
    dto::health::{ComponentHealth, HealthResponse},
    services::health as svc,
};

/// `GET /api/health` — public, unenveloped health snapshot.
///
/// Public by construction: this handler does NOT name the `AuthenticatedAccount`
/// extractor, so no auth machinery runs. The flat response shape is the
/// documented `api-contract` carve-out (see the route comment in `lib.rs`).
#[utoipa::path(
    get,
    path = "/api/health",
    responses((status = 200, description = "Health snapshot", body = HealthResponse)),
    tag = "health",
)]
pub async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    let snapshot = svc::check(&state.db).await;

    let components = vec![ComponentHealth {
        name: "db".to_string(),
        status: snapshot.db,
    }];

    let status = svc::overall_status(&components.iter().map(|c| c.status).collect::<Vec<_>>());

    Json(HealthResponse {
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: env!("GIT_COMMIT_SHA").to_string(),
        components,
    })
}

#[cfg(test)]
mod tests {
    /// Compile-time proof that `build.rs` ran and set `GIT_COMMIT_SHA`.
    /// `env!` fails to compile if the var is unset, so reaching runtime at all
    /// means it resolved; we additionally assert it is non-empty.
    #[test]
    fn git_commit_sha_is_baked_in() {
        let sha = env!("GIT_COMMIT_SHA");
        assert!(!sha.is_empty(), "GIT_COMMIT_SHA must be a non-empty string");
    }
}
