## ADDED Requirements

### Requirement: CI workflow runs the backend quality gate on backend changes

The repository SHALL include a CI workflow at `.github/workflows/backend.yml` that runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (default clippy lints, no bespoke layering rules), `cargo sqlx prepare --check` (drift check on the committed `backend/.sqlx/` offline cache), and `cargo test --all-targets` on every push and pull request that touches files under `backend/**` or the workflow file itself. The workflow SHALL provision a Postgres service container for the test step (GitHub Actions `services: postgres:16`) and SHALL set `SQLX_OFFLINE=true` job-wide so `sqlx::query!` macros compile against the committed cache rather than a live database.

#### Scenario: Workflow runs on push touching backend
- **WHEN** a commit is pushed to any branch and the commit modifies a file under `backend/**`
- **THEN** the workflow SHALL trigger and SHALL run all four checks (fmt, clippy, sqlx-prepare-check, tests) in sequence

#### Scenario: Workflow runs on pull request touching backend
- **WHEN** a pull request is opened or updated and the diff includes any file under `backend/**`
- **THEN** the workflow SHALL trigger and SHALL run all four checks; the PR SHALL be blocked from merge until the workflow passes (subject to branch-protection configuration on the repo)

#### Scenario: Workflow does not run on non-backend changes
- **WHEN** a commit modifies only files outside `backend/**` (e.g., `frontend/`, `openspec/`, `README.md`) and does not touch the workflow file
- **THEN** the workflow SHALL NOT trigger

#### Scenario: Workflow fails fast on lint violation
- **WHEN** the workflow runs and `cargo clippy --all-targets -- -D warnings` emits any error
- **THEN** the workflow SHALL exit non-zero and the subsequent test step SHALL NOT run

#### Scenario: Existing codebase passes the gate
- **WHEN** the workflow runs against `develop` after this change is implemented
- **THEN** every step (fmt, clippy, sqlx-prepare-check, tests) SHALL exit zero with no warnings

### Requirement: Backend README points at the authoritative layering source

`backend/README.md` SHALL include a section titled "Layering rules" (or equivalent) that names the `rust-rest-api` skill at `.claude/skills/rust-rest-api/SKILL.md` as the authoritative source of the backend's architectural rules, documents the pre-push command `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`, and **explicitly flags that the layering rules (handler→service→db direction, services owning no HTTP types, DTOs depending on DB models not service types) are review-enforced only** until a future change adds a mechanical gate.

#### Scenario: README points at the skill
- **WHEN** a contributor opens `backend/README.md`
- **THEN** the README SHALL identify `.claude/skills/rust-rest-api/SKILL.md` as the authoritative source of the layering rules

#### Scenario: README documents the pre-push command
- **WHEN** a contributor reads the README's "Layering rules" section
- **THEN** the section SHALL document the command(s) the contributor SHOULD run locally before pushing to keep CI green

#### Scenario: README flags the enforcement gap
- **WHEN** a contributor reads the README's "Layering rules" section
- **THEN** the section SHALL explicitly state that the layering rules are not mechanically enforced and that a future change is planned to add a mechanical gate, so a contributor is not misled into thinking clippy will catch a layering violation
