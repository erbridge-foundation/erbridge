## Context

The `rust-rest-api` skill at `.claude/skills/rust-rest-api/` defines the backend's layering rules: handlers go through services, services own no HTTP types, DTOs depend on DB models not on service types, and so on. The skill is loaded into every backend Claude session via `CLAUDE.md` and an opt-in trigger phrase, but compliance is voluntary. The foundation change has now produced a backend codebase with handlers, services, DTOs, and a DB layer, and a recent session demonstrated three concrete drift modes: a DTO importing from `crate::services`, conflict detection by string-matching SQL error messages, and missing `tests/` scaffolding. The skill itself acknowledges the gap, naming three candidate enforcement mechanisms (visibility-driven, workspace crates, clippy disallowed-types). This change picks one and wires it up.

A future `frontend-enforcement-layer` change will apply the same pattern to `frontend/` once §4 begins. The two are intentionally separate because the toolchains differ (clippy vs. eslint/stylelint) and frontend readiness lags backend by §4a wireframe approval.

## Goals / Non-Goals

**Goals:**

- Mechanically reject — at `cargo clippy` time, before review — a backend change that violates the three highest-value `rust-rest-api` skill rules: handler-imports-db, service-imports-http, and serde-derive-on-db-model.
- Run the gate in CI on every push and PR that touches `backend/**`, alongside `cargo fmt --check` and `./backend/scripts/test.sh`.
- Provide a documented, reproducible way for a future implementer to prove the gate bites (the §2d.6 / §7.27 pattern from the foundation: temporarily perturb the code, confirm the gate fails, revert).
- Stay zero-warning on the existing codebase at the moment the gate goes live.

**Non-Goals:**

- Encoding every rule in the skill. Many rules (per-layer test coverage, DTO field allowlisting semantics, response envelope shape) cannot be expressed as clippy disallowed-types and continue to rely on review + future tests. We pick the three rules with the highest signal-to-noise ratio.
- Enforcing the foundation's task list mechanically (e.g., "every handler must have a HURL test"). That belongs in a later test-scaffolding change.
- Visibility-driven enforcement via `pub(super)`. It is a defensible alternative (the skill names it first) but would require restructuring every cross-module reference in the foundation right now, mid-flight. Clippy gives 80% of the value with 10% of the churn.
- Workspace crate splitting. Strictly the most bulletproof option per the skill, but a heavyweight refactor that is out of proportion for a project this size.
- Pre-commit hooks. The user has tools to add them per `~/.claude/skills/update-config/`, but pre-commit hooks run on the developer's machine only; CI is what blocks the merge. We add the CI gate; pre-commit is the user's call to add later.
- Touching `frontend/`. Covered by the planned `frontend-enforcement-layer` change.

## Decisions

### 1. Use `clippy.toml` disallowed-types/disallowed-methods, not workspace crates or `pub(super)`

The skill names three enforcement mechanisms. We pick clippy because:

- **Reversible and incremental.** A `clippy.toml` entry can be added, observed in CI, and rolled back without touching `src/`. Workspace splitting or visibility tightening forces every cross-module path to change at once.
- **Surfaces violations at the call site, not the definition site.** A handler that adds `use crate::db::api_keys;` gets a clippy error on that exact line, with the exact rule name. Visibility violations produce an error on the imported item, which is less obvious about *why* it's forbidden.
- **Composable with the existing `cargo clippy -D warnings` idiom.** No new tool, no new flag, no new step beyond what every Rust CI already runs.

Alternatives considered: see Non-Goals.

### 2. The three rules to enforce mechanically

Per the proposal:

- `disallowed-types`: forbid `axum::Router`, `axum::extract::*`, `axum_core::response::*`, `http::*` inside files under `crate::services::*`. Concretely: a `clippy.toml` with `[[disallowed-types]]` entries lists each forbidden path; the lint fires when the type is *named*, regardless of how it got there.
- `disallowed-methods` + a `path = "crate::db::..."` entry to forbid calls into `crate::db::*` from anywhere in `crate::handlers::*`. Clippy's `disallowed-methods` lint accepts module path prefixes; we list the DB module's pub items.
- A `disallowed-macros` entry forbidding `#[derive(serde::Serialize)]` and `#[derive(serde::Deserialize)]` on items in `crate::db::*`. Clippy 1.78+ supports `disallowed-macros` directly. We do NOT add a runtime check; the lint is enough.

Each rule SHALL include an explanatory `reason` field pointing at the skill clause it encodes, so a contributor who trips the lint sees the rationale without leaving their editor.

### 3. CI venue: GitHub Actions, in `.github/workflows/backend.yml`

The repository will (per the foundation's §6 docker setup) likely live on GitHub. GitHub Actions is the default and requires no separate account or runner. The workflow:

- Triggers on `push` and `pull_request` with `paths: ['backend/**', '.github/workflows/backend.yml']`.
- Single job, `ubuntu-latest`. Steps: checkout, install Rust stable, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `./backend/scripts/test.sh`.
- The test step requires Docker. GitHub-hosted runners have Docker preinstalled; the wrapper script handles compose up/down. No additional setup needed.

If the user later moves to a different runner (Gitea, self-hosted), the workflow file is the only thing that changes; the lint config is portable.

### 4. The deliberate-violation test pattern

The foundation uses this pattern in §7.27: temporarily break the contract, confirm the test catches it, revert. We adopt the same pattern for the lint gate. A tracked file `backend/tests/enforcement_gate.md` documents three violations and the expected clippy errors. When this change is implemented, the implementer SHALL run each violation locally, paste the resulting clippy output into the file, then revert the violation. The file becomes a durable artefact proving the gate bit when the change shipped, and a regression check the user can re-run by hand.

We do NOT automate the violation itself (e.g., a `cargo test` that mutates files in `/tmp`). That would couple CI to file-system tricks and add maintenance burden disproportionate to the value. The manual reproduction step happens once, at change time, by a human (or Claude) with the spec in hand.

### 5. Out-of-scope rules that the lint cannot express

The skill includes rules clippy cannot enforce:
- "Handlers must accept injected `State<AppState>` — no globals." — there are no globals in the codebase; future violations are review-catchable.
- "Services must extend a DB method rather than add a second round-trip when possible." — semantic, not syntactic.
- "Every handler must have a HURL test." — belongs in a future test-scaffolding change with a directory-scan check.
- "DTO must not `#[serde(flatten)]` a DB model." — clippy can flag the attribute presence but not the type relationship; left to review for now.

These are listed here so the next reader understands what the gate does and does not catch.

## Risks / Trade-offs

- [Risk] **Clippy false positives on legitimate cross-module patterns.** Mitigation: each disallowed entry is keyed to a specific module path (`crate::db::*`), not a generic type (`PgPool`). A service that needs `PgPool` from `sqlx` is unaffected. If a false positive does emerge, an `#[allow(clippy::disallowed_types, reason = "...")]` with a justification is the escape hatch — the lint still fires, the suppression is auditable in PR review.
- [Risk] **The lint runs in CI but not locally; a contributor lands a PR that breaks CI.** Mitigation: documented in `backend/README.md` (this change adds the note) — run `cargo clippy --all-targets -- -D warnings` before pushing. A pre-commit hook is the user's call; outside this change's scope.
- [Risk] **Clippy version drift.** A newer clippy may add lints that fire on the existing codebase, breaking CI on a no-op rebase. Mitigation: pin the Rust toolchain via `rust-toolchain.toml` (a separate, lightweight follow-up if this becomes a problem); for now, accept the risk — the foundation runs on `rust:latest` already.
- [Risk] **The `disallowed-macros` lint requires clippy ≥1.78.** Mitigation: it stabilised in early 2024 and is on every supported stable toolchain at the time of writing. If a contributor pins an older toolchain, the lint silently no-ops on the macros rule but still catches types and methods. The README note documents the minimum version.
- [Trade-off] **Three rules vs. all of them.** A pre-commit hook or workspace split could enforce more. We chose three because they are the rules a session has actually violated and because they have a stable clippy idiom today. The gate is extensible — adding a fourth rule is a `clippy.toml` edit, not a re-design.

## Migration Plan

This is greenfield enforcement on a greenfield project. No migration of existing code is required because the codebase is already compliant on the three rules as of commit `5434b98` (the skill-compliance pass from this session ensured that). The implementer SHALL run `cargo clippy --all-targets -- -D warnings` against the head of `develop` as the first step and confirm zero warnings before adding the `clippy.toml`. If a warning appears, it is fixed under this change (the spec is "the gate passes on a clean codebase").

Rollback is trivial: delete `clippy.toml` and `.github/workflows/backend.yml`. No production behaviour, no data, no schema changes.

## Open Questions

- Should the workflow also run `cargo audit` (security advisories)? Not in this change — it is a separate concern (supply-chain) and benefits from its own cadence (daily, not per-PR). A future change can add it.
- Should the deliberate-violation file live at `backend/tests/enforcement_gate.md` or `openspec/changes/backend-enforcement-layer/violations.md`? Currently leaning on `backend/tests/` so it survives change archival. Final placement is a tasks.md decision.
- Should we also enforce "no `.unwrap()` / `.expect()` in non-test code" via clippy's `unwrap_used` / `expect_used` lints? They are noisy by default but configurable to allow `#[cfg(test)]`. Deferred to a follow-up — worth doing, but adds tuning work this change does not need.
