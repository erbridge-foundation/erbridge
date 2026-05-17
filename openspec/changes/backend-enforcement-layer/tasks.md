## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `backend/` (sections 1–4) | `rust-rest-api` | Before writing the first line of Rust or `clippy.toml` in this session |

If the skill body is not loaded and a backend task is reached, stop and load it first.

## 1. Baseline check

- [ ] 1.1 From `backend/`, run `cargo clippy --all-targets -- -D warnings` against the current `develop` HEAD. Confirm zero warnings. If any warning appears, fix it under this change before adding `clippy.toml` (per design.md §"Migration Plan").
- [ ] 1.2 From `backend/`, run `./scripts/test.sh` and confirm all tests pass against a clean clone. This establishes the baseline the CI workflow must reproduce.

## 2. Lint configuration

- [ ] 2.1 Create `backend/clippy.toml` with `[[disallowed-types]]` entries blocking, inside `crate::services::*`, the following imports: `axum::Router`, `axum::extract::State`, `axum::extract::Path`, `axum::extract::Query`, `axum::extract::Json`, `axum::response::IntoResponse`, `axum::response::Response`, `axum::http::StatusCode`, any `http::*` type. Each entry SHALL include a `reason` field referencing the `rust-rest-api` skill's "Services must not import axum, or any HTTP framework types" rule.
- [ ] 2.2 Add `[[disallowed-methods]]` entries that block, from inside `crate::handlers::*`, every public function currently exported by `crate::db::accounts`, `crate::db::api_keys`, and `crate::db::characters`. Use module-prefix paths where clippy supports them; otherwise list each fn explicitly. Each entry SHALL include a `reason` referencing the skill's "Handlers must not call db functions directly" rule.
- [ ] 2.3 Add `[[disallowed-macros]]` entries blocking `serde::Serialize` and `serde::Deserialize` derives on items under `crate::db::*`. Reason field SHALL reference the skill's "Never `#[derive(Serialize)]` directly on a DB model" rule.
- [ ] 2.4 Run `cargo clippy --all-targets -- -D warnings` from `backend/` and confirm zero output. If a real violation appears in the existing code, fix it under this change.

## 3. CI workflow

- [ ] 3.1 Create `.github/workflows/backend.yml` (at the repository root, NOT under `backend/`). Trigger: `push` on any branch and `pull_request` with `paths: ['backend/**', '.github/workflows/backend.yml']`.
- [ ] 3.2 Single job, `runs-on: ubuntu-latest`. Steps: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable` (or equivalent pinned action), then in order: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `./backend/scripts/test.sh`. Each step uses `working-directory: backend` where applicable.
- [ ] 3.3 Verify the workflow file is YAML-valid (`yamllint .github/workflows/backend.yml` or equivalent). Do NOT push yet — the next task validates behaviour first.

## 4. Prove the gate bites (deliberate-violation reference)

- [ ] 4.1 Create `backend/tests/enforcement_gate.md`. Add a heading per rule (handler→db import, service→http import, serde-on-db-model) and a brief explanation of what each rule encodes.
- [ ] 4.2 For the handler→db rule: temporarily add `use crate::db::api_keys;` to `backend/src/handlers/api/v1/keys.rs`, run `cargo clippy --all-targets -- -D warnings`, capture the full error output, paste it into the file under the corresponding heading, revert the change. Confirm `cargo clippy` is clean again after revert.
- [ ] 4.3 For the service→http rule: temporarily add `use axum::Router;` to `backend/src/services/api_keys.rs`, run clippy, capture output, paste into the file, revert, confirm clippy is clean.
- [ ] 4.4 For the serde-on-db-model rule: temporarily add `#[derive(serde::Serialize)]` to `ApiKeyRow` in `backend/src/db/api_keys.rs`, run clippy, capture output, paste into the file, revert, confirm clippy is clean.
- [ ] 4.5 Run `cargo clippy --all-targets -- -D warnings` one final time after all three reverts to confirm the working tree is clean.

## 5. Documentation

- [ ] 5.1 Create or update `backend/README.md` to include a "Layering rules" section. Content: (a) one sentence naming `backend/clippy.toml` as the load-bearing enforcement file; (b) a link/path to the `rust-rest-api` skill at `.claude/skills/rust-rest-api/SKILL.md`; (c) the command contributors SHALL run before pushing: `cargo clippy --all-targets -- -D warnings`; (d) a one-line pointer at `backend/tests/enforcement_gate.md` for the proof-of-bite reference.

## 6. Verification

- [ ] 6.1 Push the change to a feature branch. Confirm `.github/workflows/backend.yml` triggers and all three steps (fmt, clippy, tests) pass on green.
- [ ] 6.2 On the same feature branch, deliberately introduce one of the violations from §4 (e.g., the service→http import), push, and confirm the workflow fails at the clippy step with the expected error. Revert the violation in a follow-up commit on the same branch; confirm the workflow goes green again. This is the live equivalent of the foundation's §7.27 drift check.
- [ ] 6.3 Open a PR from the feature branch to `develop`. Confirm the PR cannot be merged until the workflow passes (i.e., the workflow is a required check). If branch protection is not yet configured on the repo, document the next step needed to make it a required check, but do not block the change on that — branch protection is the user's call.
- [ ] 6.4 Squash-merge the PR. The change is then ready for archival via `/openspec-archive-change`.
