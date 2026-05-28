//! Build script: bake the application version and short git commit SHA into the
//! binary at compile time, exposed as `CARGO_PKG_VERSION` / `GIT_COMMIT_SHA` and
//! read by the health handler + OpenAPI doc via `env!`.
//!
//! ## Version (`CARGO_PKG_VERSION`)
//!
//! The version is git-tag-derived and is **not** the `Cargo.toml` manifest value
//! (that is frozen at a sentinel — see RELEASING.md). When the build-time
//! `APP_VERSION` env var is set and non-empty, we override `CARGO_PKG_VERSION` so
//! both `/api/health` and the OpenAPI `info.version` report the git-derived
//! version with no handler code change. When `APP_VERSION` is unset (a plain local
//! `cargo build`), the manifest `CARGO_PKG_VERSION` is used unchanged.
//!
//! ## Commit (`GIT_COMMIT_SHA`)
//!
//! Resolution order:
//!   1. An explicit `GIT_COMMIT_SHA` build-time env var, if non-empty. This lets
//!      a build that has no `.git/` (e.g. a Docker context) still bake a real SHA
//!      by passing it in — no code change needed.
//!   2. `git rev-parse --short HEAD` against a local `.git/`.
//!   3. The literal `"unknown"`.
//!
//! The build MUST never fail because git metadata is missing — in a Docker build
//! (no `.git/` in the context) this legitimately falls back to `"unknown"`.

use std::process::Command;

fn main() {
    // Re-bake the SHA when HEAD moves (new commit, branch switch) or when an
    // explicit override is supplied.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/");
    println!("cargo:rerun-if-env-changed=GIT_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=APP_VERSION");

    // Override CARGO_PKG_VERSION from the git-derived APP_VERSION when provided;
    // otherwise leave the manifest value in place.
    if let Some(version) = app_version_override(std::env::var("APP_VERSION").ok()) {
        println!("cargo:rustc-env=CARGO_PKG_VERSION={version}");
    }

    let sha = std::env::var("GIT_COMMIT_SHA")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(git_short_sha)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_SHA={sha}");
}

fn git_short_sha() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// The version to write to `CARGO_PKG_VERSION`, given the raw `APP_VERSION` env
/// var. `Some(v)` means override with the trimmed `v`; `None` means leave the
/// manifest version untouched (env unset, empty, or whitespace-only).
///
/// NOTE: build scripts are not part of the crate's test target, so a `#[cfg(test)]`
/// module here would never run under `cargo test`. The override decision is mirrored
/// and unit-tested in `src/handlers/health.rs` (`app_version_override`); the
/// end-to-end "APP_VERSION reaches CARGO_PKG_VERSION" check is task 6.1's
/// build-and-inspect verification.
fn app_version_override(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}
