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

/// The version `build.rs` writes to `CARGO_PKG_VERSION`, given the raw build-time
/// `APP_VERSION` env var: `Some(v)` overrides with the trimmed `v`, `None` keeps the
/// `Cargo.toml` manifest value (env unset, empty, or whitespace-only).
///
/// This mirrors the decision in `build.rs`. Build scripts are not part of the test
/// target, so the logic is unit-tested here, where it compiles into the crate.
#[cfg(test)]
fn app_version_override(raw: Option<&str>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::app_version_override;

    /// Compile-time proof that `build.rs` ran and set `GIT_COMMIT_SHA`.
    /// `env!` fails to compile if the var is unset, so reaching runtime at all
    /// means it resolved; we additionally assert it is non-empty.
    #[test]
    fn git_commit_sha_is_baked_in() {
        let sha = env!("GIT_COMMIT_SHA");
        assert!(!sha.is_empty(), "GIT_COMMIT_SHA must be a non-empty string");
    }

    /// `version` on the health response reads `CARGO_PKG_VERSION`, which `build.rs`
    /// overrides from `APP_VERSION` when building for release. `env!` proves it is
    /// baked in and non-empty (the actual override-to-`1.2.3` flow is verified by a
    /// build-and-inspect step — see the change's task 6.1).
    #[test]
    fn version_is_baked_in() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(
            !version.is_empty(),
            "CARGO_PKG_VERSION must be a non-empty string"
        );
    }

    #[test]
    fn non_empty_app_version_overrides_manifest() {
        assert_eq!(
            app_version_override(Some("1.2.3")),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn app_version_is_trimmed() {
        assert_eq!(
            app_version_override(Some("  1.2.3\n")),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn unset_or_empty_app_version_keeps_manifest() {
        assert_eq!(app_version_override(None), None);
        assert_eq!(app_version_override(Some("")), None);
        assert_eq!(app_version_override(Some("   ")), None);
    }
}
