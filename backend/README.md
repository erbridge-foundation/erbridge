# erbridge backend

Rust / Axum HTTP service for the E-R Bridge wormhole mapper. Exposes `/auth/*` for EVE SSO and `/api/*` for the SvelteKit frontend.

For local-development setup (Postgres, sqlx-cli, the `.env` file, running tests) see [`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## Layering rules

The backend follows a strict handler → service → db layering. The authoritative source of these rules is the **`rust-rest-api` skill** at [`.claude/skills/rust-rest-api/SKILL.md`](../.claude/skills/rust-rest-api/SKILL.md). Read it before adding a new handler, service, or DB function.

In brief:

- `src/handlers/*` — receive the HTTP request, validate input, call exactly one service, return a DTO wrapped in the standard response envelope. Never call `src/db/*` directly.
- `src/services/*` — own all business logic; orchestrate DB calls. Never import `axum::*`, `http::*`, or any other HTTP-framework type.
- `src/db/*` — raw SQL via `sqlx::query!`. Return domain models. No business logic.
- `src/dto/*` — explicit allowlist of fields safe to put on the wire. Implement `From<DbModel>`, never `From<ServiceType>`. Never `#[derive(Serialize)]` directly on a DB model.

### Enforcement gap

> ⚠️ **The layering rules above are review-enforced only.** Clippy does NOT catch a layering violation — its `disallowed-*` lints don't support per-module scoping, and the alternative (visibility tightening or module restructure) was descoped in the `backend-enforcement-layer` change. A future change will add a mechanical gate. Until then, the rules rely on PR review and on Claude sessions loading the `rust-rest-api` skill at the start of backend work.

## Pre-push checks

Run this before pushing, so CI doesn't catch what you can catch locally:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo sqlx prepare --check -- --all-targets
cargo test --all-targets
```

`cargo sqlx prepare --check -- --all-targets` verifies the committed `backend/.sqlx/` offline cache is in sync with the `sqlx::query!` invocations in the code (the `--all-targets` flag is required so test-only invocations are included). If you've added, removed, or changed a `sqlx::query!`, regenerate the cache:

```sh
cargo sqlx prepare -- --all-targets
git add .sqlx/
```

CI runs the same four checks against a Postgres service container — see [`.github/workflows/backend.yml`](../.github/workflows/backend.yml).
