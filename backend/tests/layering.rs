//! Fail-closed layering guard.
//!
//! The architecture is handler → service → db, one-directional. These tests
//! scan the source tree and fail when a file references a layer it must not,
//! so the rule is enforced mechanically rather than by review discipline
//! (rust-rest-api skill, "Architecture: The Only Permitted Flow").
//!
//! Exceptions are listed explicitly with the reason they are blessed. Adding
//! a new exception is a deliberate, reviewable act — not a silent drift.

// Test crate: unwrap/expect are fine here (policy exempts test code).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

/// Files under `src/handlers/` permitted to reference `crate::db`.
///
/// `middleware.rs` hosts the authentication extractors. Auth resolution is a
/// pre-handler concern that deliberately reads accounts/blocks/keys without a
/// service round-trip; the auth-coverage tests pin its behaviour.
const HANDLER_DB_EXCEPTIONS: &[&str] = &["src/handlers/middleware.rs"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

/// Strips line comments so commented-out code and doc prose don't trip the scan.
fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.split("//").next().unwrap_or("")))
}

fn violations(root: &str, pattern: &str, exceptions: &[&str]) -> Vec<String> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for file in rust_files(&base.join(root)) {
        let rel = file
            .strip_prefix(base)
            .expect("file under manifest dir")
            .to_string_lossy()
            .replace('\\', "/");
        if exceptions.contains(&rel.as_str()) {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source file");
        for (line_no, line) in code_lines(&source) {
            if line.contains(pattern) {
                found.push(format!("{rel}:{line_no}: {}", line.trim()));
            }
        }
    }
    found
}

#[test]
fn handlers_do_not_call_db_directly() {
    let hits = violations("src/handlers", "crate::db", HANDLER_DB_EXCEPTIONS);
    assert!(
        hits.is_empty(),
        "handlers must go through services, not crate::db (or add a justified \
         exception in tests/layering.rs):\n{}",
        hits.join("\n")
    );
}

#[test]
fn db_layer_does_not_import_upward() {
    for pattern in ["crate::handlers", "crate::services"] {
        let hits = violations("src/db", pattern, &[]);
        assert!(
            hits.is_empty(),
            "db layer must not import from {pattern}:\n{}",
            hits.join("\n")
        );
    }
}

#[test]
fn services_do_not_import_handlers() {
    let hits = violations("src/services", "crate::handlers", &[]);
    assert!(
        hits.is_empty(),
        "services must not import from crate::handlers:\n{}",
        hits.join("\n")
    );
}

#[test]
fn services_do_not_import_axum() {
    // The skill: services must not import HTTP framework types.
    let hits = violations("src/services", "use axum", &[]);
    assert!(
        hits.is_empty(),
        "services must not import axum:\n{}",
        hits.join("\n")
    );
}
