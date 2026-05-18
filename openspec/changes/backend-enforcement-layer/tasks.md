## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `backend/` (sections 1–4) | `rust-rest-api` | Before writing the first line of Rust in this session |

If the skill body is not loaded and a backend task is reached, stop and load it first.

## 0a. Scope note — bespoke layering lint deferred

The original proposal here added a `backend/clippy.toml` enforcing handler→db, service→http, and serde-on-db-model rules. Mid-implementation that was found to be unworkable as specified (clippy's `disallowed-*` lints have no per-module scoping; the only ways to make module-scoped enforcement work require either a module restructure or a switch off clippy entirely). **The bespoke layering gate is deferred to a future change** — see proposal.md "Deferred work" and design.md "Decisions: 1. Drop the bespoke lint gate". This change now ships the CI workflow and README pointer that are independently valuable and have no dependency on the lint mechanism.

## 1. Baseline cleanup

- [x] 1.1 From `backend/`, run `cargo clippy --all-targets -- -D warnings` against the current `develop` HEAD. Fix every warning surfaced. The seven warnings found were: 3× useless `anyhow::Error` conversions in `src/handlers/auth.rs`; 1× collapsible nested `if let` in each of `src/db/accounts.rs`, `src/db/mod.rs`, `src/handlers/middleware.rs`; 1× unused `use super::*;` in a `#[cfg(test)] mod tests` block in `src/services/account.rs`. After fixing, re-run clippy and confirm zero output.
- [x] 1.2 From `backend/`, run `cargo test --all-targets` against a clean clone with the local Postgres setup described in `CONTRIBUTING.md`. Confirm all tests pass. This establishes the baseline the CI workflow must reproduce.

## 2. CI workflow

- [x] 2.1 Create `.github/workflows/backend.yml` (at the repository root, NOT under `backend/`). Trigger: `push` on any branch and `pull_request`, both with `paths: ['backend/**', '.github/workflows/backend.yml']`. _(File pre-existed from a prior commit; widened `on.push` from `branches: [main, develop]` to any-branch under this change.)_
- [x] 2.2 Single job, `runs-on: ubuntu-latest`, with a `services.postgres` block running `postgres:16` (use the default `postgres` superuser; it has `CREATEDB` and owns the `postgres` database — both required by `#[sqlx::test]`). Set `env.SQLX_OFFLINE: "true"` and `env.DATABASE_URL: postgres://postgres:postgres@localhost:5432/postgres` job-wide. Steps: `actions/checkout@v4`, `dtolnay/rust-toolchain@stable` (or equivalent pinned action), then in order: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo sqlx prepare --check` (drift check on the committed `.sqlx/` cache), `cargo test --all-targets`. Each step uses `working-directory: backend` where applicable.
- [x] 2.3 Verify the workflow file is YAML-valid (`yamllint .github/workflows/backend.yml` or equivalent). Do NOT push yet — §4 verification covers the live-run check.

## 3. Documentation

- [x] 3.1 Create `backend/README.md` (the repo doesn't have one yet). Include a "Layering rules" section with: (a) a link to `.claude/skills/rust-rest-api/SKILL.md` as the authoritative source of the backend's architectural rules; (b) a pre-push command block — `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`; (c) **an explicit note that the layering rules (handler→service→db direction, services owning no HTTP types, DTOs depending on DB models not service types) are review-enforced only — clippy will NOT catch a layering violation, a future change is planned to add a mechanical gate**. The explicit gap-flag matters: a contributor who reads the README must not be misled into thinking the gate is mechanical.
- [x] 3.2 The README MAY include additional sections relevant to the backend (e.g. local-dev setup pointing at `CONTRIBUTING.md`, a one-line description of what the backend does). Keep it brief — `CONTRIBUTING.md` is the canonical setup doc.

## 4. Verification

- [ ] 4.1 Push the change to a feature branch. Confirm `.github/workflows/backend.yml` triggers and all four steps (fmt, clippy, sqlx-prepare-check, tests) pass on green.
- [ ] 4.2 On the same feature branch, deliberately introduce one of these regressions and confirm the workflow fails at the correct step: (a) `cargo fmt`-violating whitespace → fails at fmt; (b) an unused import → fails at clippy; (c) edit a `sqlx::query!` invocation without regenerating `.sqlx/` → fails at sqlx-prepare-check; (d) a test that returns the wrong value → fails at tests. You do NOT have to run all four — one is sufficient as a smoke check. Revert in a follow-up commit; confirm the workflow goes green again.
- [ ] 4.3 Open a PR from the feature branch to `develop`. Confirm the workflow runs on the PR. If branch protection is not yet configured on the repo, document the next step needed to make the workflow a required check, but do not block the change on that — branch protection is the user's call.
- [ ] 4.4 Squash-merge the PR. The change is then ready for archival via `/openspec-archive-change`.
