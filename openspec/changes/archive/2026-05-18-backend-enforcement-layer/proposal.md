## Why

The `rust-rest-api` skill defines the backend's layering, error handling, and DTO rules, but compliance is voluntary — Claude sessions have repeatedly violated those rules (handler→service→db inversion, DTOs depending on service types, string-matching SQL error messages for conflict detection, missing test scaffolding). The skill itself flags this: *"If none of these are in place, the architecture rule is aspirational only."* The foundation change has now landed enough code that the cost of drift compounds with every new handler.

The original proposal here was to add a `clippy.toml` that mechanically rejected the three highest-value layering violations (handler→db import, service→http import, serde-derive on a DB model). **That mechanism turns out not to work**: clippy's `disallowed-types`, `disallowed-methods`, and `disallowed-macros` lints have no per-module scoping — they fire wherever the symbol appears, with no way to say "only inside `crate::services::*`". A global ban with `#[allow]` escape hatches on every legitimate caller (the only way to make clippy work for this) reduces the gate to "the human reviewer must spot a spurious `#[allow]`", which is the same review-only enforcement we already have. Visibility-driven enforcement (`pub(super)` / `pub(crate)`) also doesn't cleanly separate the layers given the current module layout: integration tests in `tests/` and DTOs in `crate::dto` both legitimately reach into `crate::db`, so a simple visibility tightening either breaks them or requires a non-trivial module restructure. (See `design.md` for the full analysis.)

This change therefore ships the parts of the original proposal that **do** deliver value without depending on the broken lint mechanism:

- **CI gate**: a GitHub Actions workflow that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo sqlx prepare --check`, and `cargo test` on every push and PR that touches `backend/**`. The clippy step still catches every default-on clippy lint — just not the bespoke layering rules.
- **README pointer**: a "Layering rules" note in `backend/README.md` naming the `rust-rest-api` skill as the authoritative source and documenting the pre-push command.
- **Baseline cleanup**: bring the existing codebase to zero clippy warnings under `-D warnings` so the CI gate goes live on a clean tree.

The mechanical layering gate is **deferred**, not dropped — see "Deferred work" below.

## What Changes

- Fix the seven existing clippy warnings in `backend/src/` (3× useless `anyhow::Error` conversions, 3× collapsible nested `if let`, 1× unused `use super::*` in a test module) so the codebase is clean under `cargo clippy --all-targets -- -D warnings`.
- New `.github/workflows/backend.yml` running `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo sqlx prepare --check`, and `cargo test --all-targets` on every push and pull request whose diff touches `backend/**` or the workflow file itself. Tests run against a Postgres service container provided by GitHub Actions (`services: postgres:16`); compile-time `sqlx::query!` validation uses the committed `backend/.sqlx/` offline cache (`SQLX_OFFLINE=true`).
- New "Layering rules" section in `backend/README.md` (created if it doesn't exist) pointing at the `rust-rest-api` skill as the authoritative source, naming the pre-push command, and explicitly flagging that the layering rules are review-enforced only until a future change adds a mechanical gate.

## Deferred work (out of scope for this change)

A future change SHALL implement mechanical layering enforcement using one of the following — to be chosen at that time on its own merits:

- **Module restructure + visibility.** Nest `services` and `db` under a shared parent (e.g. `crate::core`) so `db` can be `pub(super)` of `core` and unreachable from `crate::handlers` at compile time. Requires moving DTOs and tests to use a small re-export shim. Compiler-enforced, no tooling dependency.
- **`dylint` custom lint.** Run-time loadable lint crate that can be path-aware; gives clippy-like ergonomics with module scoping. Adds a dependency on the dylint toolchain.
- **Workspace crate split.** Move `db`, `services`, and `handlers` into separate workspace crates with explicit `Cargo.toml` dependencies. Bulletproof but heavy.

This change does **not** prejudge which option wins; it just unblocks the CI gate and the documentation so the foundation isn't waiting on the layering question.

## Capabilities

### New Capabilities

- `backend-enforcement`: CI workflow that runs fmt / clippy / sqlx-prepare-check / test on every backend-touching push and PR, and a README pointer naming `rust-rest-api` as the authoritative architecture source. The mechanical layering gate originally proposed under this capability is deferred to a future change; this capability ships the parts that are buildable today.

### Modified Capabilities

## Impact

- **Code**: 5 files under `backend/src/` get small clippy-warning fixes (auth.rs, services/account.rs, db/accounts.rs, db/mod.rs, handlers/middleware.rs). No behaviour change.
- **New files**: `.github/workflows/backend.yml`, `backend/README.md` (new file; the repo doesn't have one yet).
- **No new runtime dependencies**. `clippy` and `cargo fmt` ship with the Rust toolchain; `sqlx-cli` is already a contributor prerequisite per `CONTRIBUTING.md`.
- **Future backend changes are now blocked by CI fmt/clippy/test failures**. Default clippy lints catch a non-trivial fraction of drift even without the bespoke layering rules.
- The frontend tooling, build, and `frontend/` directory are out of scope. A separate `frontend-enforcement-layer` change will mirror the CI pattern when §4 work begins.
