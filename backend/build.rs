//! Build script: capture the short git commit SHA at compile time and expose it
//! as the `GIT_COMMIT_SHA` env var (read by the health handler via `env!`).
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
