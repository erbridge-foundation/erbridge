## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on. Each skill defines mandatory architecture, structure, and convention rules that this change relies on. Invoke via the `Skill` tool.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `backend/` (sections 2, 2b, 2c, parts of 6) | `rust-rest-api` | Before writing the first line of Rust in this session |
| Anything under `frontend/` (sections 4a, 4, 5) | `sveltekit-node` | Before writing the first line of Svelte / TypeScript in `frontend/` in this session. §4a wireframes are plain HTML and do NOT require this skill, but they must be approved before §4 begins. |

If you (Claude) reach a backend or frontend task and the relevant skill body has not been loaded in this session, stop and invoke it first. Loading both up-front is fine; they are independent.

## 0a. Prior-session warnings (read before starting)

A previous Sonnet session implemented §2b but introduced two `rust-rest-api` skill violations that were fixed in commit `5434b98`. The violations and their fixes are documented in that commit message; the corrected patterns are what is in `develop` now. The implementer of remaining tasks MUST NOT reintroduce either pattern:

1. **DTOs MUST implement `From<DbModel>`, not `From<ServiceType>`.** A DTO importing from `crate::services::*` is a layering inversion. The skill rule lives under "DTOs" — re-read it before adding any new DTO. Concretely: `src/dto/keys.rs` was previously `impl From<services::KeyMetadata> for KeyMetadataDto`; it is now `impl From<db::ApiKeyMetadata> for KeyMetadataDto`. Mirror that direction for every new DTO.

2. **Conflict detection MUST match on a typed `DbError` variant, not on `e.to_string().contains("unique")`.** String-matching SQL error messages is fragile and was explicitly fixed. The `DbError::UniqueViolation { constraint }` variant in `src/db/mod.rs` is the canonical pattern; `sqlx::Error` already converts into it via the `From` impl in the same file. New DB functions that can hit a unique constraint SHOULD return `Result<_, DbError>` and let the conversion handle the mapping.

A third class of issue — missing `backend/tests/` scaffolding (integration + HURL per the `rust-rest-api` skill) — has not yet been addressed and is still open. It will be picked up by a future change; do NOT bundle it into §2c work. Each new handler added under §2c SHOULD still have its `#[cfg(test)]` unit tests per the skill's coverage rules.

Mechanical enforcement of these rules (clippy + CI) is queued as the `backend-enforcement-layer` change — see `openspec/changes/backend-enforcement-layer/`. Until it lands, the gate is review + this notice.

## 1. Repository Scaffold

- [x] 1.1 Create root-level `frontend/` and `backend/` directories
- [x] 1.2 Write `.env.example` with `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET`, `DATABASE_URL`, `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` — no secret defaults, comments for each
- [x] 1.3 Add top level `.gitignore` file covering `.env`, `.idea/`, `.vscode/`, `.DS_Store`, `*Zone.Identifier`, `zz-ref/`
- [x] 1.3 Add component level `.gitignore` files
- [x] 1.3.1 Backend using standard Rust .gitignore from `https://github.com/github/gitignore/blob/main/Rust.gitignore`
- [x] 1.4.2 Frontend using the following .gitignore verbatim
```
# Playwright
playwright-report/

node_modules
dist
test-results/
package-lock.json
yarn.lock
vite.config.js.timestamp-*
/packages/package/test/**/package
/documentation/types.js
.vercel_build_output
.svelte-kit
.cloudflare
.pnpm-debug.log
.netlify
.turbo
.vercel
.test-tmp
symlink-from
_tmp_flaky_test_output.txt
```

## 2. Backend: Rust/Axum Project

- [x] 2.1 Initialise Cargo project in `backend/` (`cargo init`)
- [x] 2.2 Add dependencies to `Cargo.toml`: `axum`, `tokio` (full), `reqwest` (json feature), `serde`/`serde_json`, `thiserror`, `anyhow`, `aes-gcm`, `jsonwebtoken`, `uuid` (v7 + serde features), `tower-http` (cors/trace), `dotenvy`, `sqlx` (postgres + runtime-tokio-rustls + uuid + chrono + macros features), `chrono` (serde feature)
- [x] 2.3 Implement `backend/src/esi/mod.rs` with `EsiMetadata` struct and `discover()` function verbatim as specified
- [x] 2.4 Implement `backend/src/config.rs`: read `APP_URL`, `ENCRYPTION_SECRET`, `ESI_CLIENT_ID`, `ESI_CLIENT_SECRET`, `DATABASE_URL` from env; fail fast with clear error if any are missing
- [x] 2.5 Create `backend/migrations/00000000000001_create_account_eve_character_and_api_key.sql`: `CREATE EXTENSION IF NOT EXISTS pgcrypto;` then `CREATE TABLE account (...)`, `CREATE TABLE eve_character (...)`, and `CREATE TABLE api_key (...)` matching the schema in design.md §3a verbatim, including all indexes (`account_server_admin_idx`, `eve_character_one_main_per_account`, `eve_character_account_id_idx`, `api_key_hash_idx`, `api_key_account_idx`). Table names MUST be singular.
- [x] 2.6 Implement `backend/src/db/mod.rs`: `connect(database_url: &str) -> Result<PgPool>` that creates a pool with a bounded initial-connection retry, then runs `sqlx::migrate!("./migrations").run(&pool).await`
- [x] 2.7 Implement `backend/src/db/accounts.rs`:
  - `create_account() -> Result<Uuid>` — inserts a row with defaults, returns `id`
  - `get_account(id) -> Result<Option<Account>>` — returns the row including `status`, `delete_requested_at`, `is_server_admin`
  - `reactivate_if_soft_deleted(tx, id)` — sets `status = 'active'`, `delete_requested_at = NULL` only when `status = 'soft_deleted'`; takes a transaction so it can be atomic with character upsert
  - `soft_delete(id)` — sets `status = 'soft_deleted'`, `delete_requested_at = now()`
- [x] 2.8 Implement `backend/src/db/characters.rs`. The SSO callback composes these as separate steps inside a single transaction (see §2.13); each step is independently unit-testable per the `rust-rest-api` skill's coverage requirement:
  - `upsert_tokens(tx, resolved_account_id, eve_character_id, name, corporation_id, alliance_id, esi_client_id, access_token_plaintext, refresh_token_plaintext, expires_at) -> Result<Uuid>` — encrypts both tokens with fresh nonces, then performs `INSERT ... ON CONFLICT (eve_character_id) DO UPDATE` with the rule: if existing `account_id IS NULL` (orphan claim) OR matches `resolved_account_id`, set `account_id = excluded.account_id` and rewrite tokens + public info; otherwise leave `account_id` unchanged but still update public info and tokens (re-login on owned row). Bumps `updated_at`. Returns the row's internal UUID. **Does NOT touch `is_main`** — promotion is `promote_if_no_main`'s job.
  - `promote_if_no_main(tx, account_id, just_written_character_id) -> Result<bool>` — `UPDATE eve_character SET is_main = TRUE WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM eve_character WHERE account_id = $2 AND is_main = TRUE)`. Returns whether the row was promoted. Safe to call unconditionally after `upsert_tokens` — it is a no-op when an `is_main` row already exists for the account.
  - `create_orphan(eve_character_id, name, corporation_id, alliance_id) -> Result<Uuid>` — inserts a row with `account_id = NULL` and NULL token columns.
  - `list_for_account(account_id) -> Result<Vec<Character>>` — returns characters (no decrypted tokens).
  - `delete_character(id) -> Result<bool>` — hard `DELETE`; returns whether a row was deleted.
  - `set_main(tx, account_id, character_id) -> Result<()>` — in one transaction step, clears existing `is_main` on the account then sets it on the target. Used by the `POST /api/v1/characters/:id/set-main` handler (§2c.5). May surface a unique-violation if two callers race; the handler maps that to a 409 and the partial unique index `eve_character_one_main_per_account` is the ultimate guard.
- [x] 2.9 Implement `backend/src/session.rs`: `Session` struct (`session_id: String`, `account_id: Uuid`, `csrf_state: Option<String>`, `add_character_mode: bool`); `SessionStore` (`Arc<RwLock<HashMap<String, Session>>>`); add/remove/get helpers. Sessions do NOT hold token material — tokens live in Postgres only.
- [x] 2.10 Implement `backend/src/handlers/crypto.rs`: AES-256-GCM encrypt/decrypt helpers for ESI tokens at rest (`encrypt_token` returns `nonce || ciphertext || tag` packed BYTEA; `decrypt_token` inverse) and for the session cookie payload; HS256 sign/verify for session cookie JWT; all keyed from `ENCRYPTION_SECRET`
- [x] 2.11 Implement `backend/src/handlers/cookie.rs`: helpers to create and clear the `httpOnly`, `SameSite=Lax`, `Path=/` session cookie
- [x] 2.12 Implement `backend/src/handlers/auth.rs`:
  - `GET /auth/login` handler — build EVE SSO redirect URL from `EsiMetadata.authorization_endpoint`, include CSRF state, redirect.
  - Accept the OPTIONAL `?return_to=<path>` query parameter. Validate per the spec: the value MUST start with a single `/`, MUST NOT start with `//` or `/\\`, and MUST NOT contain `\r` or `\n`. Stash the validated value alongside the CSRF state in the in-flight session record. Invalid values are silently dropped (callback then redirects to `/`).
  - Implement the validator as a small helper `pub(crate) fn validate_return_to(raw: &str) -> Option<String>` so the same logic is reused by `/auth/characters/add` and is unit-testable.
- [x] 2.13 Implement `GET /auth/callback` handler. Validate state, exchange code for tokens via `EsiMetadata.token_endpoint`, parse access-token JWT for `eve_character_id` and `name`; fetch `corporation_id` / `alliance_id` from ESI public-info. Then, in a single Postgres transaction, compose these DB functions in this order:

  1. `let account_id = accounts::resolve_or_create(&mut tx, session.add_character_account_id, eve_character_id).await?;` — returns the session's account when in add-character mode; otherwise either the account that already owns this `eve_character_id`, or a newly-created account row. Implement in `backend/src/db/accounts.rs` as a sibling of the existing helpers.
  2. `accounts::reactivate_if_soft_deleted(&mut tx, account_id).await?;` — already specified in §2.7.
  3. `let character_id = characters::upsert_tokens(&mut tx, account_id, eve_character_id, name, corp_id, alliance_id, esi_client_id, access, refresh, expires).await?;`
  4. `characters::promote_if_no_main(&mut tx, account_id, character_id).await?;` — no-op when the account already has a main; promotes the just-written character otherwise. This is the implementation of the "First linked character is promoted to main" scenario in eve-sso-auth.

  Commit. Insert/replace the in-memory session pointing to the resolved `account_id`. Set cookie. Redirect to the stashed `return_to` path if any, otherwise `/`. Each composition step is a service-layer call returning typed results; no step bundles unrelated concerns.
- [x] 2.14 Implement `GET /auth/logout` handler: remove session from store, clear cookie, redirect to `/`
- [x] 2.15 Implement `GET /auth/characters/add` handler: require existing session (401 if absent), mark the session as `add_character_mode = true`, accept and validate the same OPTIONAL `?return_to=<path>` parameter via `validate_return_to`, redirect to EVE SSO; the shared `/auth/callback` handler reads `add_character_mode` to decide whether to reuse the session's account and honours the stashed `return_to`.
- [x] 2.16 Wire `AppState` in `backend/src/main.rs`: load config, `db::connect()` (which runs migrations), call `discover()` at startup (exit on failure for any of these), initialise `SessionStore`, build Axum router with all `/auth/*` routes
- [x] 2.17 Verify `cargo build --release` produces zero warnings (use `SQLX_OFFLINE=true` with `cargo sqlx prepare` checked in, or rely on a running DB at build time — pick one and document). Resolved: `.sqlx/` query cache is checked in, so offline builds work without a database. Confirmed zero warnings at commit `5434b98`.

## 2b. Backend: API key authentication

- [x] 2b.1 Add `sha2` (or `ring`) and `base64` crates to `Cargo.toml`
- [x] 2b.2 Implement `backend/src/handlers/api_key.rs` (key generation/hashing helpers — not a route handler, but lives in the handler layer as a support module):
  - `pub const PREFIX: &str = "erb_";`
  - `pub fn generate() -> String` — draw 32 bytes from a CSPRNG, base64url-encode unpadded (43 chars), return `format!("{PREFIX}{body}")`
  - `pub fn hash(key: &str) -> String` — SHA-256 hex digest of the full key
- [x] 2b.3 Implement `backend/src/db/api_keys.rs`:
  - `create_account_key(account_id, name, expires_at) -> Result<(Uuid, String)>` — generates a key, inserts the row with `scope = 'account'`, returns `(id, plaintext_key)`. Plaintext exists only in the return value.
  - `lookup_by_key(plaintext: &str) -> Result<Option<ApiKeyRow>>` — `SELECT ... WHERE key_hash = $1 AND (expires_at IS NULL OR expires_at > now())`
  - `list_for_account(account_id) -> Result<Vec<ApiKeyMetadata>>` — no `key_hash` in the returned shape
  - `delete_for_account(id, account_id) -> Result<bool>` — `DELETE ... WHERE id = $1 AND account_id = $2`, returns whether a row was deleted
- [x] 2b.4 Implement `backend/src/handlers/middleware.rs`: an Axum extractor / middleware `AuthenticatedAccount(pub Uuid)`. On `/api/*`:
  1. If `Authorization: Bearer <value>` is present and starts with `erb_`: look up via `lookup_by_key`. On hit with `scope = 'account'` → set `account_id`; with `scope = 'server'` → reject 403; miss/expired → reject 401.
  2. Else fall back to session cookie. If neither → 401.
- [x] 2b.5 Implement `backend/src/handlers/api/v1/keys.rs`:
  - `POST /api/v1/keys` — body `{ name, expires_at? }`; calls `create_account_key`; returns `201` with `id, key, name, expires_at, created_at`
  - `GET /api/v1/keys` — calls `list_for_account` for the caller's account
  - `DELETE /api/v1/keys/:id` — calls `delete_for_account`; `204` on success, `404` otherwise (row not found OR belongs to another account OR `scope = 'server'`)
- [x] 2b.6 Mount the `/api/v1/keys` routes behind the `AuthenticatedAccount` middleware in `backend/src/main.rs`
- [x] 2b.7 Verify with `curl`: create a key via session cookie; use the returned plaintext as `Authorization: Bearer …` to list keys; delete it; subsequent requests with that key return 401


## 2c. Backend: Account-management endpoints

- [x] 2c.1 Implement `backend/src/esi/public_info.rs`: `fetch_corporation_name(corporation_id) -> Result<String>` and `fetch_alliance_name(alliance_id) -> Result<String>` against the ESI public-info endpoints discovered via the existing `EsiMetadata` flow (or the documented ESI base URL — pick one and note it). Both functions take `&reqwest::Client`; no caching in this change.
- [x] 2c.2 Extend `backend/src/db/characters.rs` with read/check helpers used by the new `/api/v1/characters/*` handlers (the write functions `upsert_tokens`, `promote_if_no_main`, and `set_main` were defined in §2.8 and are reused here):
  - `count_for_account(account_id) -> Result<i64>` — for the `cannot_remove_last_character` check
  - `is_main(id) -> Result<Option<(Uuid, bool)>>` — returns `(account_id, is_main)` so the handler can verify ownership and main-status in one query. Returns `None` when no row matches.
  - The "first linked character is promoted to main" behaviour lives in `promote_if_no_main` (§2.8) and is called from the SSO callback (§2.13). It is NOT re-implemented here; the `POST /api/v1/characters/:id/set-main` handler (§2c.5) calls `set_main` directly and does not go through the promote-if-no-main path.
- [x] 2c.3 Extend `backend/src/db/accounts.rs`: `soft_delete` already exists from 2.7 — wire it into a new handler entry point. Add `list_sessions_for_account(account_id)` helper on `SessionStore` (or equivalent) so the soft-delete handler can drop every session belonging to the soft-deleted account.
- [x] 2c.4 Implement `backend/src/handlers/api/v1/me.rs`:
  - `GET /api/v1/me` — load the caller's `account` row + all `eve_character` rows; for each character, resolve `corporation_name` and (when `alliance_id IS NOT NULL`) `alliance_name` via `esi::public_info`; build the response shape from `account-management/spec.md` (no token fields included). Wrap in the success envelope per `api-contract`.
- [x] 2c.5 Implement `backend/src/handlers/api/v1/characters.rs`:
  - `POST /api/v1/characters/:id/set-main` — verify the character belongs to the caller (404 otherwise); call `set_main` in a transaction; reload and return the updated character (same shape as one element of `GET /api/v1/me`'s `characters` array, including resolved corp/alliance names and `portrait_url`).
  - `DELETE /api/v1/characters/:id` — verify ownership (404 otherwise); if `is_main = true` and the account has >1 character → 409 `cannot_remove_main`; if it is the only character → 409 `cannot_remove_last_character`; otherwise hard-delete the row and return 204.
- [x] 2c.6 Implement `backend/src/handlers/api/v1/account.rs`:
  - `DELETE /api/v1/account` — in a single Postgres transaction call `accounts::soft_delete(caller.account_id)`. After commit, drop every in-memory session belonging to that account. Set a session-cookie-clearing `Set-Cookie` header on the response. Return 204.
  - Extend the auth middleware (or the per-route guard) so that an `Authorization: Bearer erb_…` whose `account.status = 'soft_deleted'` is rejected with HTTP 401 and `error.code = "account_soft_deleted"` (per account-management spec).
- [x] 2c.7 Mount the new routes behind the `AuthenticatedAccount` middleware in `backend/src/main.rs` alongside `/api/v1/keys`.
- [ ] 2c.8 Verify with `curl`: `GET /api/v1/me` returns the expected shape after login; `POST /api/v1/characters/<id>/set-main` flips `is_main`; `DELETE /api/v1/characters/<main_id>` returns 409 while siblings exist; `DELETE /api/v1/characters/<only_id>` returns 409; `DELETE /api/v1/account` returns 204 and the cookie is cleared.

## 2d. Backend: OpenAPI document via `utoipa` (strict)

These tasks discharge the api-contract spec's "Machine-readable API description" requirement. They depend on §2b and §2c being implemented first (the handlers and DTOs are what get annotated).

- [ ] 2d.1 Add `utoipa` and `utoipa-swagger-ui` (with the `axum` feature on `utoipa-swagger-ui`) to `backend/Cargo.toml`. Pin to the latest stable major.
- [ ] 2d.2 Derive `utoipa::ToSchema` on every request/response DTO in `backend/src/dto/` and on the success-envelope (`ApiResponse<T>`) and error-envelope (`ApiError`) types. The envelope types SHALL be declared once and referenced from every annotated response, not inlined per-route.
- [ ] 2d.3 Annotate every `/api/v1/*` handler with `#[utoipa::path(...)]`:
  - `POST /api/v1/keys`, `GET /api/v1/keys`, `DELETE /api/v1/keys/:id` (from §2b)
  - `GET /api/v1/me`, `POST /api/v1/characters/:id/set-main`, `DELETE /api/v1/characters/:id`, `DELETE /api/v1/account` (from §2c)
  - Each annotation SHALL declare: HTTP method, path, request body schema (where applicable), one response per status code returned (including 2xx, 4xx, and 5xx envelopes), `security` requirements (session cookie or bearer), and the canonical `error.code` values it may return as part of the 4xx response descriptions.
- [ ] 2d.4 Create `backend/src/openapi.rs`: a struct with `#[derive(utoipa::OpenApi)]` listing every annotated path and every component schema (DTOs + envelopes). Set `info.title = "E-R Bridge API"`, `info.version` from `CARGO_PKG_VERSION`.
- [ ] 2d.5 Mount routes in `backend/src/main.rs`:
  - `GET /api/openapi.json` — returns `ApiDoc::openapi().to_json()?` with `Content-Type: application/json`.
  - `GET /api/docs` — Swagger UI bound to `/api/openapi.json`, via `utoipa_swagger_ui::SwaggerUi`.
  - Both routes SHALL be public (no auth) so external clients and the Swagger UI can fetch them. They live outside the `AuthenticatedAccount` middleware tree.
- [ ] 2d.6 Write `backend/tests/openapi_strict.rs`: an integration test that asserts every documented route's actual response validates against its declared schema. Concrete implementation:

  **Dependencies** (dev-dependencies in `Cargo.toml`): `jsonschema = "0.17"` (pin major; widen later if needed), `serde_json = "1"`, `tower = { version = "0.4", features = ["util"] }` for `oneshot` on the Axum router.

  **Setup**: factor router construction into `pub fn build_router(state: AppState) -> Router` in `backend/src/main.rs`, so the test calls it with a test `AppState` (mocked DB pool via the project's standard `MockDb` trait per the `rust-rest-api` skill).

  **Schema extraction**: walk the `utoipa::openapi::OpenApi` value once at the start of the test:

  ```rust
  let doc = backend::openapi::ApiDoc::openapi();
  let doc_json: serde_json::Value = serde_json::to_value(&doc).unwrap();
  let components = doc_json.pointer("/components/schemas").unwrap().clone();

  // For each (path, method, status) we want to test:
  fn schema_for(
      doc_json: &serde_json::Value,
      path: &str,
      method: &str,
      status: &str,
  ) -> serde_json::Value {
      let raw = doc_json
          .pointer(&format!(
              "/paths/{}/{}/responses/{}/content/application~1json/schema",
              path.replace('/', "~1"),
              method,
              status
          ))
          .expect("response schema present in doc")
          .clone();
      raw // $ref will be resolved by jsonschema against components below
  }

  // Compile with components as the resolution root so $refs work:
  let compiled = jsonschema::JSONSchema::options()
      .with_document(
          "http://erb.local/openapi#".to_string(),
          serde_json::json!({ "components": { "schemas": components } }),
      )
      .compile(&schema_for(&doc_json, "/api/v1/me", "get", "200"))
      .unwrap();
  ```

  **Per-route assertions**: define a `cases: &[Case]` slice covering every documented `(path, method, status, request_fn, expected_status)` tuple — minimally a 200 and one representative 4xx per route. For each case:

  1. `router.clone().oneshot(request_fn()).await.unwrap()` — send the request through the real router.
  2. Assert `response.status() == expected_status`.
  3. Read the response body to JSON.
  4. Compile the schema for `(path, method, expected_status)` and validate the body against it. On failure, panic with `panic!("OpenAPI drift on {method} {path} -> {status}: {errors:?}")`.

  **Failure mode**: this is the "strict drift" check. A handler whose response diverges from its annotation fails CI. To prove it bites, §7.27 temporarily perturbs one handler and confirms this test fails.

- [ ] 2d.7 Add a doc-coverage test in the same file:

  ```rust
  let documented: HashSet<(String, String)> = doc_json
      .pointer("/paths").unwrap().as_object().unwrap()
      .iter()
      .flat_map(|(path, methods)| {
          methods.as_object().unwrap().keys()
              .map(move |m| (path.clone(), m.clone()))
      })
      .collect();

  let registered = backend::main::registered_api_v1_routes(); // expose a helper that returns Vec<(path, method)>

  for r in &registered {
      assert!(documented.contains(r), "route {r:?} is registered but missing from OpenAPI doc");
  }
  ```

  A handler with no `#[utoipa::path]` annotation SHALL fail this test. `registered_api_v1_routes` lives in `backend/src/main.rs` and is built from the same route table the router is built from, so it cannot drift from the router itself.
- [ ] 2d.8 Verify with `curl`: `curl $APP_URL/api/openapi.json | jq .openapi` returns `"3.1.0"` (or the version `utoipa` emits — note in the test if it's `3.0.x`); `curl $APP_URL/api/docs` returns HTML with `<title>Swagger UI</title>`.

## 3. Backend: Dockerfile

- [ ] 3.1 Write `backend/Dockerfile`: multi-stage build — `rust:latest` builder stage compiling release binary, then `debian:bookworm-slim` runtime stage copying binary
- [ ] 3.2 Ensure `EXPOSE 3000` (or chosen internal port) and `CMD` are set correctly

## 4a. Wireframes (author and approve BEFORE frontend implementation)

These wireframes are the authoritative visual contract for section 4 (per design.md §10). Each file SHALL be a standalone, self-contained HTML page that can be opened in a browser with no build step. They SHALL inline the design tokens from design.md §10 (`--space-*`, slate scale, `--sky`, `--emerald`, `--red`) on `:root` and load JetBrains Mono from Google Fonts, so the rendered HTML is visually accurate to within ~5% of the final Svelte implementation. Use realistic placeholder data ("Wasp 223", "Artemisia de'Halicarnass", "Exit-Strategy", "Unchained Alliance") so the layout is reviewable.

The user reviews and approves these wireframes before any task in section 4 begins. Tweaks in this phase are cheap; tweaks after Svelte implementation are not.

- [ ] 4a.1 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/login.html` matching screenshot 01: centred card on `--space-950` background, cyan sun logo, `E-R BRIDGE` wordmark, "Wormhole Mapper" subtitle, divider, official EVE SSO PNG button (`https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png`), two-line disclaimer in `--slate-500`.
- [ ] 4a.2 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/home.html` matching screenshot 02: 48px GlobalNav at top with logo + brand + `maps` + `characters` links + pulsing emerald `connected` dot + user chip (24px portrait + name + chevron); centred-left body content reading `Welcome, Wasp 223` in `--sky`, then the main character name + `main` badge + corporation/alliance lines. No "Map view coming soon." line — that placeholder lives on `/maps`.
- [ ] 4a.3 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/maps.html`: same GlobalNav as `home.html` with `maps` shown as the active link; centred body with a single line `Map view coming soon.` in `--slate-500`. The wireframe SHALL also include one **layout-level error banner** example per design.md §14: a strip in `--red` directly beneath the GlobalNav reading "Couldn't load your account: <error.message>", so the banner's visual treatment is locked in. The maps page is a fine host for this example since the other pages will rely on the same shared component. This is the placeholder for the future map-rendering change.
- [ ] 4a.4 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/characters.html` matching screenshot 04: same GlobalNav with `characters` shown as the active link; page heading `CHARACTERS` in `--slate-500` uppercase + `+ add character` button top-right (`href="/auth/characters/add?return_to=/characters"`); two stacked character cards (one with `main` badge and no actions, one with right-aligned `set main` and `remove` text buttons); horizontal divider; `DANGER ZONE` section with `delete account` text button in `--red`. The wireframe SHALL also include one example **error state** per design.md §14: render the non-main card a second time below the main list with a `--red` inline error message ("Couldn't remove character: cannot_remove_main") anchored directly under it, so the visual treatment of inline form-action errors is locked in.
- [ ] 4a.5 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/user-menu.html` matching screenshot 06: render `home.html`'s top-right chrome with the user-menu dropdown open beneath the user chip. Items: `preferences` (greyed-out, `aria-disabled="true"`), `settings` (greyed-out, `aria-disabled="true"`), divider, `log out` (`href="/auth/logout"`). Card surface is `--space-900` with `--space-700` border, anchored to the chip's right edge.
- [ ] 4a.6 The user opens each wireframe in a browser and signs off. If anything looks wrong (spacing, copy, palette, hover states, dropdown anchoring, badge style), the wireframe is updated before section 4 begins. The implementer SHALL NOT proceed to section 4 with un-approved wireframes.

  **Spec-sync rule**: design.md §11 ("Visible UI surface") and §14 ("UI surface for API errors") describe load-bearing behaviour (which character drives the user chip, what `maps` resolves to, where errors surface). If wireframe review changes any of those decisions — e.g. you decide errors should be toasts after all, or `maps` should be `/` not `/maps`, or `delete account` should require a confirmation step — the implementer SHALL update the relevant section of design.md *and* re-run `openspec validate eve-wormhole-mapper-foundation` before §4 begins. The wireframes and design.md MUST agree at approval time; downstream tasks (and the rust-rest-api / sveltekit-node skills) read both, so they can't drift.

  Cosmetic-only changes (spacing, font weight, exact shade of red, hover transitions, badge corner radius) do NOT require a design.md update — they live entirely in the wireframe.

## 4. Frontend: SvelteKit Project

All tasks in this section are blocked on §4a approval. Each task SHALL produce output that matches the corresponding wireframe; if a wireframe is silent on something (e.g. exact hover colour), it falls back to design.md §10/§11.

- [ ] 4.1 Scaffold SvelteKit app in `frontend/` using `npm create svelte@latest` with Svelte 5 and TypeScript
- [ ] 4.2 Install `@sveltejs/adapter-node`; update `svelte.config.js` to use it
- [ ] 4.3 Create `frontend/src/app.css`: import JetBrains Mono from Google Fonts; define all design-system CSS custom properties (`--space-950` through `--space-600`, full slate scale, `--sky`, `--emerald`, `--amber`, `--red`) on `:root` — names and values MUST match the wireframes' inlined tokens exactly so the Svelte build is pixel-equivalent; set `html, body` to `background: var(--space-950); color: var(--slate-100); font-family: "JetBrains Mono", ui-monospace, monospace; font-size: 13px`; import in `+layout.svelte`.
- [ ] 4.4 Create `frontend/src/lib/api.ts`: typed wrapper around the backend's `/api/v1/me`, `/api/v1/characters/:id/set-main`, `/api/v1/characters/:id`, and `/api/v1/account` endpoints. Each function returns the unwrapped `data` payload on success and throws a typed error for non-2xx responses (carrying `error.code` and `error.message` from the envelope). Types are hand-typed in this change to mirror the backend OpenAPI doc at `/api/openapi.json`; a future change wires in `openapi-typescript` (or similar) to generate them. Every type in this file SHALL include a `// keep in sync with: backend/src/dto/<file>.rs` comment pointing at the backend source.
- [ ] 4.4a Declare `App.Locals` in `frontend/src/app.d.ts`: add `interface Locals { me: MeResponse | null }` so `event.locals.me` is typed end-to-end in `+layout.server.ts`, `+page.server.ts`, and form actions. `MeResponse` is imported from `frontend/src/lib/api.ts`.
- [ ] 4.5 Implement `frontend/src/routes/+layout.server.ts`: on every request, call backend `GET /api/v1/me` (forwarding the `cookie` header). On 401, set `event.locals.me = null` and redirect to `/login` unless the current route is `/login`. On 200, store the response in `event.locals.me`. If the request targets `/login` while `event.locals.me` is set, redirect to `/`.

  **Lifecycle**: `event.locals.me` is populated *only* by this layout load, and only for the duration of a single SvelteKit request. It is NOT a long-lived store, NOT a session, and is NEVER set by client-side code or by hook code outside this layout. After the SSO callback (`/auth/callback` on the backend, not in SvelteKit) redirects the browser to `/` or to `return_to`, that subsequent request goes through this layout again and `locals.me` is repopulated from the now-valid session cookie. There is no path by which `locals.me` carries state across requests; each request fetches `/api/v1/me` afresh. This keeps the auth gate trivial: the cookie is the only durable state, and the layout is the single point that reads it.

  **Errors**: 401 → redirect to `/login` (no banner). Network error or 5xx → set `locals.me = null` AND return `{ me: null, meError: { code, message } }` so the layout can render the top-of-page error banner per design.md §14. The page still renders (degraded). 4xx other than 401 → same as 5xx (treated as recoverable, banner shown).
- [ ] 4.6 Create `frontend/src/lib/components/GlobalNav.svelte` matching `wireframes/home.html`'s top bar exactly: 48px bar (`background: var(--space-900)`, `border-bottom: 1px solid var(--space-700)`); brand logo SVG in `--sky` + `E-R BRIDGE` wordmark linking to `/`; nav links `maps` (→ `/maps`) and `characters` (→ `/characters`), active link in `--sky` with a `--space-700` pill; right side: pulsing `--emerald` status dot + `connected` label (red + `disconnected` if `me` load failed); to its right, the `UserChip` component (4.7).
- [ ] 4.7 Create `frontend/src/lib/components/UserChip.svelte`: 24px circular portrait of the **main** character + main character name + chevron caret. Click toggles a `UserMenu` (4.8). Closes on outside-click and `Escape`.
- [ ] 4.8 Create `frontend/src/lib/components/UserMenu.svelte` matching `wireframes/user-menu.html`: dropdown card (`--space-900` background, `--space-700` border, anchored to the chip's right edge); items `preferences` (`aria-disabled="true"`, `tabindex="-1"`, no hover), `settings` (same), `--space-700` divider, `log out` (`<a href="/auth/logout">`).
- [ ] 4.9 Create `frontend/src/routes/+layout.svelte`: full-height shell (`display: flex; flex-direction: column; height: 100vh; overflow: hidden; background: var(--space-950)`); include `GlobalNav` at top **unless** the current route is `/login` (login has no nav per wireframe); when `data.meError` is set (the `me` fetch failed with non-401), render the layout-level error banner described in design.md §14 directly below the GlobalNav; yield `{@render children()}` for main content.
- [ ] 4.10 Implement `frontend/src/routes/login/+page.svelte` matching `wireframes/login.html`: full-viewport `--space-950` background; centred card (`background: var(--space-900)`, `border: 1px solid var(--space-700)`, `border-radius: 8px`); E-R Bridge logo SVG in `--sky`; wordmark + "Wormhole Mapper" subtitle; `<hr>` divider; `<a href="/auth/login"><img src="https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png" alt="LOG IN with EVE Online"></a>`; two-line disclaimer in `--slate-500`.
- [ ] 4.11 Implement `frontend/src/routes/+page.server.ts`: re-use `event.locals.me` from the layout (no separate fetch); pass `me` to the page as `data.me`.
- [ ] 4.12 Implement `frontend/src/routes/+page.svelte` (Svelte 5 syntax) matching `wireframes/home.html`: centred-left content area; `Welcome, <main.name>` heading in `--sky`; main character name + `main` badge + corporation row + alliance row (alliance row omitted when `alliance_id IS NULL`). No sidebar. No canvas. No "Map view coming soon." line — that placeholder lives on `/maps`. The sidebar from the original design is deferred to the future map-rendering change.
- [ ] 4.13 Implement `frontend/src/routes/maps/+page.svelte` matching `wireframes/maps.html`: centred body with a single line `Map view coming soon.` in `--slate-500`. No `+page.server.ts` needed; `event.locals.me` from the layout is enough to gate access. This is the placeholder destination for the `maps` nav link.
- [ ] 4.14 Implement `frontend/src/routes/characters/+page.server.ts`: re-use `event.locals.me`; return `{ characters: locals.me.characters }`. Also expose form actions, each of which returns `fail(status, { code, message })` on the envelope's `error.code` / `error.message` so the page can surface them inline per design.md §14:
  - `setMain` — `POST /api/v1/characters/:id/set-main` then invalidate the layout load
  - `remove` — `DELETE /api/v1/characters/:id` then invalidate; surface `error.code` (`cannot_remove_main`, `cannot_remove_last_character`) as a user-visible message anchored to the character's card
  - `deleteAccount` — `DELETE /api/v1/account` then `redirect(303, '/login')` on success; on failure, surface the error inline under the DANGER ZONE button
- [ ] 4.15 Implement `frontend/src/routes/characters/+page.svelte` matching `wireframes/characters.html`: page heading `CHARACTERS` + `+ add character` link (`href="/auth/characters/add?return_to=/characters"`); stacked character cards rendered from `data.characters` (portrait from `portrait_url`, name, `main` badge for the main, corporation row, alliance row when present, `set main` + `remove` actions on non-main cards using the form actions from 4.14); inline error rendered in `--red` directly below a card when its action's `form.code` is set, with `data-error-code={form.code}` attached so tests can branch on the code; divider; `DANGER ZONE` section with `delete account` form action button and an inline error slot underneath it. Errors surface inline per design.md §14; no toasts and no `+error.svelte` route.
- [ ] 4.16 Verify `npm run build` produces a `build/` directory with a runnable Node.js server.
- [ ] 4.17 Open each implemented page side-by-side with its wireframe in a browser; the layouts SHALL be visually equivalent. Tune spacing, colours, and hover states until they match. Document any deliberate deviations as a comment in the relevant `+page.svelte`.

## 5. Frontend: Dockerfile

- [ ] 5.1 Write `frontend/Dockerfile`: multi-stage build — `node:lts` builder stage running `npm ci && npm run build`, then slim runtime stage running `node build`
- [ ] 5.2 Ensure `EXPOSE 3000` (or chosen port) and `CMD ["node", "build"]` are set

## 6. Traefik + Postgres Configuration

- [ ] 6.1 Write `traefik.yml` (static config): enable Docker provider, set entrypoint on port 80, disable dashboard in production
- [ ] 6.2 Write `docker-compose.yml` with four services:
  - `traefik`: mounts `/var/run/docker.sock` and `traefik.yml`; publishes port 80
  - `postgres`: `postgres:16` image; env vars `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` from `.env`; named volume `postgres-data:/var/lib/postgresql/data`; healthcheck using `pg_isready`
  - `backend`: built from `./backend`; env vars from `.env` including `DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}`; `depends_on: postgres: condition: service_healthy`; Traefik labels routing `/auth/` and `/api/` to it
  - `frontend`: built from `./frontend`; Traefik label routing all other requests to it
- [ ] 6.3 Declare the `postgres-data` named volume at the top level of `docker-compose.yml`
- [ ] 6.4 Confirm routing rules use `PathPrefix` matchers and that backend rule has higher priority than frontend catch-all

## 7. Integration Verification

- [ ] 7.1 Copy `.env.example` to `.env`, fill in real EVE SSO credentials and generated `ENCRYPTION_SECRET`
- [ ] 7.2 Run `docker compose up --build`; confirm all four containers reach running state and backend logs report migrations applied
- [ ] 7.3 Navigate to `APP_URL/`; confirm redirect to `/login`
- [ ] 7.4 Click the EVE SSO button; complete the SSO flow; confirm redirect to `/` shows the character name
- [ ] 7.5 Connect to Postgres (`docker compose exec postgres psql -U $POSTGRES_USER $POSTGRES_DB`); confirm one row in `account` (`status = 'active'`, `is_server_admin = false`) and one row in `eve_character` with non-NULL `encrypted_access_token`, `encrypted_refresh_token`, `esi_token_expires_at`, `esi_client_id`, `corporation_id`
- [ ] 7.6 Visit `/auth/characters/add`; complete SSO with a second EVE character; confirm a second row in `eve_character` linked to the same `account_id`
- [ ] 7.7 Run `docker compose down && docker compose up`; log in again with the same character; confirm the existing `eve_character` row is updated (same `id`, new `updated_at`, new ciphertext bytes due to fresh nonce) rather than duplicated
- [ ] 7.8 Soft-delete sanity: temporarily issue `UPDATE account SET status='soft_deleted', delete_requested_at = now()`; log in again; confirm `status` returns to `'active'` and `delete_requested_at` is NULL
- [ ] 7.9 Visit `APP_URL/auth/logout`; confirm redirect to `/login`
- [ ] 7.10 Verify session cookie is `httpOnly` and `SameSite=Lax` in browser devtools
- [ ] 7.11 Verify no token material (access or refresh) appears in any cookie or response header, and that plaintext tokens never appear in backend logs
- [ ] 7.12 API key end-to-end: while authenticated by session cookie, `curl -X POST $APP_URL/api/v1/keys -d '{"name":"smoke","expires_at":null}'`; capture returned plaintext `key`; confirm it matches `erb_[A-Za-z0-9_-]{43}`; confirm one row in `api_key` with `scope = 'account'`, matching `account_id`, and `key_hash` equal to `echo -n "$KEY" | sha256sum | cut -d' ' -f1`
- [ ] 7.13 API key authenticates: `curl -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys`; confirm the response lists the key and does NOT include a plaintext field
- [ ] 7.14 API key revocation: `curl -X DELETE -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys/$ID`; confirm 204; repeat the previous `curl` with the same key and confirm 401
- [ ] 7.15 Visit `/login` in a browser: layout matches `wireframes/login.html` (logo, wordmark, subtitle, EVE SSO button, disclaimer). The page has no GlobalNav.
- [ ] 7.16 After SSO, the browser lands on `/` and the layout matches `wireframes/home.html`: GlobalNav with brand + `maps` + `characters` + emerald `connected` dot + user chip (main character's portrait + name + chevron); body shows `Welcome, <main name>` plus the main character block plus `Map view coming soon.`
- [ ] 7.17 Click the user chip: dropdown appears matching `wireframes/user-menu.html` (`preferences` and `settings` greyed-out and non-interactive; divider; `log out`). Click outside or press `Escape`: dropdown closes. Click `log out`: redirected to `/login`.
- [ ] 7.18 Click `maps` in the nav: layout matches `wireframes/maps.html` (GlobalNav with `maps` active, body shows `Map view coming soon.`). Click `characters` in the nav: layout matches `wireframes/characters.html` with `characters` active. The main character's card has no actions; the non-main card has `set main` and `remove`. Click `+ add character`: redirected to EVE SSO; after completing with a third character, the new row appears in the list **and the browser returns to `/characters`** (not `/`) because the `?return_to=/characters` hint was honoured. The previous main is unchanged.
- [ ] 7.19 Click `set main` on a non-main character: the page reloads, the badge moves, and the user chip in the GlobalNav now shows the new main's portrait and name.
- [ ] 7.20 Attempt to `remove` the main character via direct API call (`curl -X DELETE`): confirm 409 `cannot_remove_main`. With only one character linked, attempt to remove it: confirm 409 `cannot_remove_last_character`.
- [ ] 7.21 Click `delete account` in the DANGER ZONE: response is 204, session cookie is cleared, browser redirects to `/login`. Confirm in Postgres that `account.status = 'soft_deleted'` and `delete_requested_at` is set; `eve_character` rows are untouched.
- [ ] 7.22 Log in again as the soft-deleted account's character: per existing 7.8, the account is reactivated. Visit `/`: the home page renders normally.
- [ ] 7.23 With a soft-deleted account (before re-login), attempt to use a previously-issued API key: confirm 401 with `error.code = "account_soft_deleted"`.
- [ ] 7.24 Open the rendered pages side-by-side with their wireframes. Any deviation that is not documented as deliberate is treated as a bug and fixed before this change is considered complete.
- [ ] 7.25 `GET /api/openapi.json` returns HTTP 200 with `Content-Type: application/json` and a body whose `openapi` field is `"3.1.0"` (or the version `utoipa` emits — record it in the test). The document includes every `/api/v1/*` route mounted by the backend.
- [ ] 7.26 `GET /api/docs` renders a Swagger UI page (`<title>Swagger UI</title>` in the HTML; the JSON model URL points at `/api/openapi.json`).
- [ ] 7.27 The backend `cargo test` suite passes including the strict-drift test (§2d.6) and the doc-coverage test (§2d.7). To prove the drift test bites, temporarily change one handler's response shape without updating its annotation and confirm the test fails; revert.
- [ ] 7.28 `GET /auth/login?return_to=/characters` followed by SSO completion redirects the browser to `/characters` (not `/`). Repeat with `?return_to=https://evil.example.com/` and confirm the callback redirects to `/` (off-origin rejected). Repeat with `?return_to=//evil.example.com/` and confirm the callback redirects to `/` (scheme-relative rejected).
- [ ] 7.29 **Pre-archival**: move `openspec/changes/eve-wormhole-mapper-foundation/wireframes/` to `frontend/wireframes/` so the wireframes survive archival as a tracked, durable artefact. Update any references in the archived design.md note to point at the new location. Delete `zz-ref/frontend/screenshots/` — the wireframes have superseded them. Run this task only when all earlier §7 checks have passed and the change is ready to be archived.
