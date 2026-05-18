## Context

The `rust-rest-api` skill at `.claude/skills/rust-rest-api/` defines the backend's layering rules: handlers go through services, services own no HTTP types, DTOs depend on DB models not on service types, and so on. The skill is loaded into every backend Claude session via `CLAUDE.md` and an opt-in trigger phrase, but compliance is voluntary. The foundation change has now produced a backend codebase with handlers, services, DTOs, and a DB layer, and a recent session demonstrated three concrete drift modes: a DTO importing from `crate::services`, conflict detection by string-matching SQL error messages, and missing `tests/` scaffolding. The skill itself acknowledges the gap, naming three candidate enforcement mechanisms (visibility-driven, workspace crates, clippy disallowed-types). This change originally picked clippy; mid-implementation that choice was found to be unworkable (see "Decision: drop the bespoke lint gate" below). This change now ships the CI gate and documentation that are buildable today, and defers the mechanical layering gate to a future change.

A future `frontend-enforcement-layer` change will apply the same CI pattern to `frontend/` once §4 begins. The two are intentionally separate because the toolchains differ (clippy vs. eslint/stylelint) and frontend readiness lags backend by §4a wireframe approval.

## Goals / Non-Goals

**Goals (revised):**

- Run a baseline backend quality gate in CI on every push and PR that touches `backend/**`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (default clippy lints only, no bespoke layering rules), `cargo sqlx prepare --check`, and `cargo test --all-targets`.
- Land the gate on a clean tree (zero clippy warnings on the existing codebase).
- Document `rust-rest-api` as the authoritative architecture source in `backend/README.md` so a contributor without the skill loaded has a pointer.

**Non-Goals (revised):**

- **Mechanical layering enforcement at lint time.** Originally a goal; demoted to "deferred work" after the implementation analysis below. The layering rules remain enforced by review + the `rust-rest-api` skill alone until a future change picks a workable mechanism.
- Encoding every rule in the skill. Many rules (per-layer test coverage, DTO field allowlisting semantics, response envelope shape) cannot be expressed as clippy disallowed-types and continue to rely on review + future tests.
- Enforcing the foundation's task list mechanically (e.g., "every handler must have a HURL test"). That belongs in a later test-scaffolding change.
- Pre-commit hooks. The user has tools to add them per `~/.claude/skills/update-config/`, but pre-commit hooks run on the developer's machine only; CI is what blocks the merge. We add the CI gate; pre-commit is the user's call to add later.
- Touching `frontend/`. Covered by the planned `frontend-enforcement-layer` change.

## Decisions

### 1. Drop the bespoke lint gate; defer mechanical layering enforcement

The original plan was a `clippy.toml` with three rules: forbid `axum::*`/`http::*` inside `crate::services::*`, forbid `crate::db::*` calls inside `crate::handlers::*`, and forbid `serde::Serialize` derives inside `crate::db::*`. Mid-implementation, this turned out to be unbuildable as specified:

- **Clippy's `disallowed-types` / `disallowed-methods` / `disallowed-macros` lints have no per-module scoping.** They fire wherever the symbol appears, with no `inside = "crate::services::*"` predicate. The `reason` field is a string shown in the error message, not a scope filter.
- **The only way to make clippy work for this is a global ban with `#[allow]` escape hatches on every legitimate caller** (in `services/`, `dto/`, `main.rs`, and the integration `tests/` crate). That reduces the gate to "the human reviewer must spot a spurious `#[allow]`", which is the same review-only enforcement we already have.
- **Visibility-driven enforcement (`pub(super)` / `pub(crate)`) — the skill's first-named option — doesn't cleanly separate the layers given the current module layout.** `crate::dto` legitimately imports from `crate::db` (skill: "DTOs MUST implement `From<DbModel>`"), and integration tests in `tests/` call `backend::db::accounts::create_account` directly. A simple tightening of `db`'s visibility either breaks those callers or requires a non-trivial module restructure (e.g., nesting `services` + `db` under a shared parent and moving DTOs / re-exporting for tests).

Given two not-yet-implemented changes (`persist-sessions-postgres` and the foundation's remaining frontend work) are higher priority than a layering gate, **we descope the lint gate from this change** and defer it. The deferred work has three candidate mechanisms, listed in the proposal under "Deferred work"; the next change to take this up SHALL pick one on its own merits.

### 2. Keep the CI gate and the README note

These two pieces of the original plan are independently valuable and have no dependency on the lint mechanism:

- **CI gate.** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` (with default lints only) catch a real fraction of drift — formatting regressions, common Rust footguns, dead code, suspicious patterns. `cargo sqlx prepare --check` catches `.sqlx/` cache drift. `cargo test --all-targets` catches functional regressions. None of these depend on bespoke layering rules.
- **README note.** Currently no `backend/README.md` exists; this change creates one. The "Layering rules" section names the `rust-rest-api` skill as the authoritative source, documents `cargo clippy --all-targets -- -D warnings` as the pre-push command, and **explicitly flags that the layering rules are review-enforced only** until a future change adds a mechanical gate. The explicit flag is important — a contributor who reads the README must not be misled into thinking clippy will catch a layering violation.

### 3. CI venue: GitHub Actions, in `.github/workflows/backend.yml`

The repository will (per the foundation's §6 docker setup) live on GitHub. GitHub Actions is the default and requires no separate account or runner. The workflow:

- Triggers on `push` and `pull_request` with `paths: ['backend/**', '.github/workflows/backend.yml']`.
- Single job, `ubuntu-latest`, with `services: postgres:16` (the workflow uses the default `postgres` superuser, which already has `CREATEDB` and owns the `postgres` database — both required by `#[sqlx::test]`). Steps: checkout, install Rust stable, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo sqlx prepare --check`, `cargo test --all-targets`.
- `SQLX_OFFLINE=true` is set job-wide so `sqlx::query!` macros validate against the committed `backend/.sqlx/` cache rather than needing a live DB at build time. `DATABASE_URL` is set so `cargo test` can talk to the service container at runtime.

If the user later moves to a different runner (Gitea, self-hosted), the workflow file is the only thing that changes.

### 4. Baseline cleanup before the gate goes live

Running `cargo clippy --all-targets -- -D warnings` against `develop` at the start of this change surfaced seven warnings (3× useless `anyhow::Error` conversions in `src/handlers/auth.rs`, 1× collapsible nested `if let` in each of `src/db/accounts.rs`, `src/db/mod.rs`, and `src/handlers/middleware.rs`, and 1× unused `use super::*;` in a `#[cfg(test)] mod tests` block in `src/services/account.rs`). All seven are trivial mechanical fixes with no behaviour change. They are fixed under this change so the CI gate goes live on a clean tree.

### 5. Out-of-scope rules that the gate does not catch

This change's gate enforces formatting, default clippy lints, `.sqlx/` cache freshness, and test pass/fail. It does NOT catch:

- Handler→db imports (deferred).
- Service→HTTP imports (deferred).
- `serde::Serialize` derives on DB models (deferred).
- "Handlers must accept injected `State<AppState>` — no globals." (review-only.)
- "Services must extend a DB method rather than add a second round-trip when possible." (semantic; review-only.)
- "Every handler must have a HURL test." (future test-scaffolding change.)
- "DTO must not `#[serde(flatten)]` a DB model." (review-only.)

These are listed here so the next reader understands what the gate does and does not catch, and so the deferred-work change has a clear scope.

## Risks / Trade-offs

- [Risk] **The layering rules remain voluntary until the deferred change lands.** Mitigation: the README note explicitly flags this and points at the skill. The next layering violation in a PR is caught by review, as before — no regression vs. today, but no improvement either on that axis.
- [Risk] **The CI gate runs in CI but not locally; a contributor lands a PR that breaks CI.** Mitigation: documented in `backend/README.md` — run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` before pushing. A pre-commit hook is the user's call; outside this change's scope.
- [Risk] **Clippy version drift.** A newer clippy may add lints that fire on the existing codebase, breaking CI on a no-op rebase. Mitigation: pin the Rust toolchain via `rust-toolchain.toml` (a separate, lightweight follow-up if this becomes a problem); for now, accept the risk — the foundation runs on `rust:latest` already.
- [Trade-off] **Ship CI now vs. wait for the layering gate.** We chose ship-now because the CI gate is independently valuable (catches `.sqlx/` drift, formatting regressions, test breakage, and default-clippy footguns) and the layering question deserves its own design pass rather than a forced answer here.

## Migration Plan

Greenfield enforcement on a greenfield project. No migration of existing code is required because the codebase is already compliant on every other axis as of the foundation change. The seven clippy warnings noted in Decision 4 are fixed as the first task. Rollback is trivial: delete `.github/workflows/backend.yml` and the README section. No production behaviour, no data, no schema changes.

## Open Questions

- Should the workflow also run `cargo audit` (security advisories)? Not in this change — it is a separate concern (supply-chain) and benefits from its own cadence (daily, not per-PR). A future change can add it.
- Which mechanism does the deferred layering gate pick — module restructure, `dylint`, or workspace split? Out of scope here; left to the change that takes that work up.
- Should we also enforce "no `.unwrap()` / `.expect()` in non-test code" via clippy's `unwrap_used` / `expect_used` lints? They are noisy by default but configurable to allow `#[cfg(test)]`. Deferred to a follow-up — worth doing, but adds tuning work this change does not need.
