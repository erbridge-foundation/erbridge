## ADDED Requirements

### Requirement: Clippy lint config enforces backend layering rules

The backend SHALL ship a `backend/clippy.toml` configuration that mechanically enforces the highest-value layering rules from the `rust-rest-api` skill via clippy's `disallowed-types`, `disallowed-methods`, and `disallowed-macros` lints. Each entry SHALL include a `reason` field pointing at the skill clause it encodes.

#### Scenario: Handler importing from the DB layer is rejected
- **WHEN** a file under `backend/src/handlers/` introduces `use crate::db::api_keys` (or any other `crate::db::*` path) and `cargo clippy --all-targets -- -D warnings` is run
- **THEN** clippy SHALL emit a `disallowed-methods` (or equivalent) error pointing at the import, with a reason text referencing the `rust-rest-api` skill's "Handlers must not call db functions directly" rule

#### Scenario: Service importing HTTP types is rejected
- **WHEN** a file under `backend/src/services/` introduces `use axum::Router`, `use axum::extract::Json`, `use axum_core::response::IntoResponse`, or any `http::*` symbol and `cargo clippy --all-targets -- -D warnings` is run
- **THEN** clippy SHALL emit a `disallowed-types` error pointing at the import, with a reason text referencing the skill's "Services must not import axum, or any HTTP framework types" rule

#### Scenario: serde derive on a DB model is rejected
- **WHEN** a struct under `backend/src/db/` is annotated with `#[derive(serde::Serialize)]` (or `Deserialize`) and `cargo clippy --all-targets -- -D warnings` is run
- **THEN** clippy SHALL emit a `disallowed-macros` error on the derive, with a reason text referencing the skill's "Never `#[derive(Serialize)]` directly on a DB model" rule

#### Scenario: Existing codebase passes the gate
- **WHEN** `cargo clippy --all-targets -- -D warnings` is run against `develop` after this change is implemented
- **THEN** clippy SHALL exit with status 0 and no warnings or errors

### Requirement: CI workflow runs the gate on backend changes

The repository SHALL include a CI workflow at `.github/workflows/backend.yml` (or equivalent venue named in `design.md`) that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `./backend/scripts/test.sh` on every push and pull request that touches files under `backend/**` or the workflow file itself.

#### Scenario: Workflow runs on push touching backend
- **WHEN** a commit is pushed to any branch and the commit modifies a file under `backend/**`
- **THEN** the workflow SHALL trigger and SHALL run all three checks (fmt, clippy, tests) in sequence

#### Scenario: Workflow runs on pull request touching backend
- **WHEN** a pull request is opened or updated and the diff includes any file under `backend/**`
- **THEN** the workflow SHALL trigger and SHALL run all three checks; the PR SHALL be blocked from merge until the workflow passes

#### Scenario: Workflow does not run on non-backend changes
- **WHEN** a commit modifies only files outside `backend/**` (e.g., `frontend/`, `openspec/`, `README.md`) and does not touch the workflow file
- **THEN** the workflow SHALL NOT trigger

#### Scenario: Workflow fails fast on lint violation
- **WHEN** the workflow runs and clippy emits any error
- **THEN** the workflow SHALL exit non-zero and the subsequent test step SHALL NOT run

### Requirement: Deliberate-violation reference proves the gate bites

The repository SHALL include a tracked reference file at `backend/tests/enforcement_gate.md` that documents one deliberate violation per enforced rule, with the verbatim clippy error message produced when that violation is introduced. The implementer of this change SHALL produce the error messages by temporarily introducing each violation locally, capturing clippy's output, and reverting the change before committing the reference file.

#### Scenario: Reference file lists all three rule violations
- **WHEN** a reader opens `backend/tests/enforcement_gate.md`
- **THEN** the file SHALL contain three sections — one per enforced rule (handler→db import, service→http import, serde-on-db-model) — each showing the violating code snippet and the captured clippy error output

#### Scenario: Reference file is reproducible
- **WHEN** a reader manually introduces any of the documented violations and runs `cargo clippy --all-targets -- -D warnings`
- **THEN** clippy SHALL produce an error matching the captured output in the reference file (modulo line numbers and absolute paths)

### Requirement: Backend README points at the lint config

`backend/README.md` SHALL include a section titled "Layering rules" (or equivalent) that names `backend/clippy.toml` as the load-bearing source of the backend's architectural rules and links to the `rust-rest-api` skill for the rationale. The section SHALL instruct contributors to run `cargo clippy --all-targets -- -D warnings` before pushing.

#### Scenario: README explains the gate
- **WHEN** a contributor opens `backend/README.md`
- **THEN** the README SHALL clearly identify `clippy.toml` as the enforcement boundary, link to the `rust-rest-api` skill, and document the local pre-push command
