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

A third class of issue — missing `backend/tests/` scaffolding (integration + HURL per the `rust-rest-api` skill) — has since been addressed: `backend/tests/openapi_strict.rs` and `backend/tests/api_keys.rs` cover integration via `#[sqlx::test]`, and `backend/tests/hurl/` holds live HTTP contract tests (`me.hurl`, `keys.hurl`, `characters.hurl`, `account.hurl`). New handlers SHALL extend this scaffolding rather than ship without integration coverage.

The CI quality gate (fmt / clippy / sqlx-prepare-check / test on every backend push and PR) shipped via the `backend-enforcement-layer` change — see `openspec/changes/archive/2026-05-18-backend-enforcement-layer/` for the original proposal and the three deferred options (module restructure, dylint, workspace split) for the mechanical *layering* gate. The layering rules remain review-enforced until that future change lands; default clippy lints catch a non-trivial fraction of drift in the meantime.

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
- [x] 2.5 Create `backend/migrations/00000000000001_create_account_eve_character_and_api_key.sql`: `CREATE EXTENSION IF NOT EXISTS pgcrypto;` then `CREATE TABLE account (...)`, `CREATE TABLE eve_character (...)`, and `CREATE TABLE api_key (...)` matching the schema in design.md §3a verbatim, including all indexes (`account_server_admin_idx`, `eve_character_one_main_per_account`, `eve_character_account_id_idx`, `api_key_hash_idx`, `api_key_account_idx`). Table names MUST be singular. Sibling migration `00000000000002_create_session.sql` creates the `session` table and its `session_expires_at_idx` / `session_account_id_idx` indexes per design.md §3a — see also §2.5b. **Schema note:** the foundation change has been amended twice since this task was first implemented and not yet shipped; §2c.10 edits the `0001` migration in place to (a) add `corporation_name TEXT NOT NULL` and `alliance_name TEXT` for the denormalisation refactor, (b) rename `esi_token_expires_at` to `access_token_expires_at` for honesty about which token's expiry the column records, and (c) add `scopes TEXT[] NOT NULL DEFAULT '{}'` so a future `missing_scopes` `token_status` does not require a migration.
- [x] 2.5b Create `backend/migrations/00000000000002_create_session.sql` containing the `session` table from design.md §3a (TEXT PK, `account_id` FK with `ON DELETE CASCADE`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, `expires_at`) and the two indexes `session_expires_at_idx` and `session_account_id_idx`. Singular table name per project convention.
- [x] 2.6 Implement `backend/src/db/mod.rs`: `connect(database_url: &str) -> Result<PgPool>` that creates a pool with a bounded initial-connection retry, then runs `sqlx::migrate!("./migrations").run(&pool).await`
- [x] 2.7 Implement `backend/src/db/accounts.rs`:
  - `create_account() -> Result<Uuid>` — inserts a row with defaults, returns `id`
  - `get_account(id) -> Result<Option<Account>>` — returns the row including `status`, `delete_requested_at`, `is_server_admin`
  - `reactivate_if_soft_deleted(tx, id)` — sets `status = 'active'`, `delete_requested_at = NULL` only when `status = 'soft_deleted'`; takes a transaction so it can be atomic with character upsert
  - `soft_delete(id)` — sets `status = 'soft_deleted'`, `delete_requested_at = now()`
- [x] 2.8 Implement `backend/src/db/characters.rs`. The SSO callback composes these as separate steps inside a single transaction (see §2.13); each step is independently unit-testable per the `rust-rest-api` skill's coverage requirement:
  - `upsert_tokens(tx, resolved_account_id, eve_character_id, name, corporation_id, corporation_name, alliance_id, alliance_name, esi_client_id, access_token_plaintext, refresh_token_plaintext, access_token_expires_at, scopes) -> Result<Uuid>` — encrypts both tokens with fresh nonces, then performs `INSERT ... ON CONFLICT (eve_character_id) DO UPDATE` with the rule: if existing `account_id IS NULL` (orphan claim) OR matches `resolved_account_id`, set `account_id = excluded.account_id` and rewrite tokens + public info; otherwise leave `account_id` unchanged but still update public info and tokens (re-login on owned row). `corporation_name` is required (`NOT NULL` column); `alliance_name` is required when `alliance_id` is `Some`, and must be `None` when `alliance_id` is `None`. `scopes: &[String]` is the array parsed from the access-token JWT's `scp` claim and is persisted to the `scopes` `TEXT[]` column. Bumps `updated_at`. Returns the row's internal UUID. **Does NOT touch `is_main`** — promotion is `promote_if_no_main`'s job.
  - `promote_if_no_main(tx, account_id, just_written_character_id) -> Result<bool>` — `UPDATE eve_character SET is_main = TRUE WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM eve_character WHERE account_id = $2 AND is_main = TRUE)`. Returns whether the row was promoted. Safe to call unconditionally after `upsert_tokens` — it is a no-op when an `is_main` row already exists for the account.
  - `create_orphan(eve_character_id, name, corporation_id, corporation_name, alliance_id, alliance_name) -> Result<Uuid>` — inserts a row with `account_id = NULL` and NULL token columns. Same name/ID pairing rule as `upsert_tokens`.
  - `list_for_account(account_id) -> Result<Vec<Character>>` — returns characters (no decrypted tokens).
  - `delete_character(id) -> Result<bool>` — hard `DELETE`; returns whether a row was deleted.
  - `set_main(tx, account_id, character_id) -> Result<()>` — in one transaction step, clears existing `is_main` on the account then sets it on the target. Used by the `POST /api/v1/characters/:id/set-main` handler (§2c.5). May surface a unique-violation if two callers race; the handler maps that to a 409 and the partial unique index `eve_character_one_main_per_account` is the ultimate guard.
- [x] 2.9 Implement `backend/src/db/sessions.rs` and `backend/src/session.rs`:
  - `backend/src/db/sessions.rs` (per the `rust-rest-api` skill's `db/` layout) — `insert(pool, session_id, account_id, csrf_state, add_character_mode) -> Result<()>` inserts with `expires_at = now() + interval '7 days'`; `refresh_and_get(pool, session_id) -> Result<Option<Session>>` runs a single `UPDATE session SET last_seen_at = now(), expires_at = now() + interval '7 days' WHERE session_id = $1 AND expires_at > now() RETURNING *` and returns `None` when no row matched (missing or expired); `delete(pool, session_id) -> Result<()>`; `list_ids_for_account(pool, account_id) -> Result<Vec<String>>` (non-expired rows only); `delete_expired(pool) -> Result<u64>` for opportunistic cleanup. Unit tests via `#[sqlx::test]` cover insert+read, refresh-advances-timestamps, refresh-of-expired-returns-None-and-leaves-row, delete, list-excludes-expired, delete-expired.
  - `backend/src/session.rs` — `Session` struct (`session_id: String`, `account_id: Uuid`, `csrf_state: Option<String>`, `add_character_mode: bool`, plus `created_at` / `last_seen_at` / `expires_at: chrono::DateTime<Utc>`). `SessionStore` is a thin wrapper holding a `PgPool` (NOT `Arc<RwLock<HashMap<…>>>` — that was an earlier draft; sessions are Postgres-backed per design.md §3 / §3d so they survive backend restarts). Public methods `new(PgPool)`, `add`, `get`, `remove`, `list_session_ids_for_account` delegate to `db::sessions::*`. `get` calls `refresh_and_get` so reading a session is also what extends it. Sessions do NOT hold token material — tokens live in `eve_character`.
  - In-flight OAuth2 records (csrf_state + return_to for a single redirect cycle, no `account_id` yet) live in a sibling `InflightStore` (in-memory `HashMap`) — they are intentionally restart-volatile.
- [x] 2.10 Implement `backend/src/handlers/crypto.rs`: AES-256-GCM encrypt/decrypt helpers for ESI tokens at rest (`encrypt_token` returns `nonce || ciphertext || tag` packed BYTEA; `decrypt_token` inverse) and for the session cookie payload; HS256 sign/verify for session cookie JWT; all keyed from `ENCRYPTION_SECRET`
- [x] 2.11 Implement `backend/src/handlers/cookie.rs`: helpers to create and clear the `httpOnly`, `SameSite=Lax`, `Path=/` session cookie
- [x] 2.12 Implement `backend/src/handlers/auth.rs`:
  - `GET /auth/login` handler — build EVE SSO redirect URL from `EsiMetadata.authorization_endpoint`, include CSRF state, redirect.
  - Accept the OPTIONAL `?return_to=<path>` query parameter. Validate per the spec: the value MUST start with a single `/`, MUST NOT start with `//` or `/\\`, and MUST NOT contain `\r` or `\n`. Stash the validated value alongside the CSRF state in the in-flight session record. Invalid values are silently dropped (callback then redirects to `/`).
  - Implement the validator as a small helper `pub(crate) fn validate_return_to(raw: &str) -> Option<String>` so the same logic is reused by `/auth/characters/add` and is unit-testable.
- [x] 2.13 Implement `GET /auth/callback` handler. Validate state, exchange code for tokens via `EsiMetadata.token_endpoint`, parse access-token JWT for `eve_character_id`, `name`, and the granted `scp` (scopes); fetch `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name` from ESI public-info via `esi::public_info` (§2c.1) — both ID and name for each entity, so the persisted row carries the resolved string. The `scp` claim in EVE's access-token JWT may be either a single string or an array of strings depending on how many scopes were granted; deserialise via a `#[serde(untagged)]` enum `Scp { One(String), Many(Vec<String>) }` and normalise to a `Vec<String>` before passing through to `upsert_tokens`. Then, in a single Postgres transaction, compose these DB functions in this order:

  1. `let account_id = accounts::resolve_or_create(&mut tx, session.add_character_account_id, eve_character_id).await?;` — returns the session's account when in add-character mode; otherwise either the account that already owns this `eve_character_id`, or a newly-created account row. Implement in `backend/src/db/accounts.rs` as a sibling of the existing helpers.
  2. `accounts::reactivate_if_soft_deleted(&mut tx, account_id).await?;` — already specified in §2.7.
  3. `let character_id = characters::upsert_tokens(&mut tx, account_id, eve_character_id, name, corp_id, corp_name, alliance_id, alliance_name, esi_client_id, access, refresh, expires, &scopes).await?;`
  4. `characters::promote_if_no_main(&mut tx, account_id, character_id).await?;` — no-op when the account already has a main; promotes the just-written character otherwise. This is the implementation of the "First linked character is promoted to main" scenario in eve-sso-auth.

  Commit. Insert a fresh `session` row via `db::sessions::insert` (or update the existing one in add-character mode) pointing to the resolved `account_id`, with `csrf_state` and `add_character_mode` carried from the in-flight OAuth2 record and `expires_at = now() + interval '7 days'`. Set the session cookie carrying an HS256 JWT of the session ID with `exp` matching the DB row. Redirect to the stashed `return_to` path if any, otherwise `/`. Each composition step is a service-layer call returning typed results; no step bundles unrelated concerns.
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
  1. If `Authorization: Bearer <value>` is present and starts with `erb_`: look up via `lookup_by_key`. On hit with `scope = 'account'` → set `account_id`; with `scope = 'server'` → reject 403; miss/expired → reject 401. Bearer auth does NOT touch the `session` table and does NOT cause a session-cookie refresh.
  2. Else fall back to session cookie. Decode the cookie JWT for the session ID, then call `SessionStore::get` (which delegates to `db::sessions::refresh_and_get` — a single `UPDATE … WHERE expires_at > now() RETURNING *` that atomically advances `last_seen_at` / `expires_at` and rejects expired rows). On `None` → 401. On `Some` → set `account_id`, AND record on a request-scoped slot that a fresh session-cookie JWT (`exp = now() + 7 days`) should be set on the response. A `tower` middleware layer (`refresh_session_cookie`, installed in `main.rs`) reads the slot on response and writes the `Set-Cookie` header (see persist-sessions-postgres §5 archive for the original implementation rationale).
  3. If neither → 401.
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
  - `list_for_account` (already defined in §2.8) SHALL select `corporation_name` and `alliance_name` from the row alongside the existing columns so the `/api/v1/me` handler can build its response without further DB or network calls.
  - The "first linked character is promoted to main" behaviour lives in `promote_if_no_main` (§2.8) and is called from the SSO callback (§2.13). It is NOT re-implemented here; the `POST /api/v1/characters/:id/set-main` handler (§2c.5) calls `set_main` directly and does not go through the promote-if-no-main path.
- [x] 2c.3 Extend `backend/src/db/accounts.rs`: `soft_delete` already exists from 2.7 — wire it into a new handler entry point. The `SessionStore` already exposes `list_session_ids_for_account` (which delegates to `db::sessions::list_ids_for_account`, defined in §2.9), so the soft-delete handler uses that pair plus `remove` to drop every session belonging to the soft-deleted account from the `session` table.
- [x] 2c.4 Implement `backend/src/handlers/api/v1/me.rs`:
  - `GET /api/v1/me` — load the caller's `account` row + all `eve_character` rows; for each character, resolve `corporation_name` and (when `alliance_id IS NOT NULL`) `alliance_name` via `esi::public_info`; build the response shape from `account-management/spec.md` (no token fields included). Wrap in the success envelope per `api-contract`. **NOTE:** the per-request ESI fan-out implemented here is removed by §2c.10; the handler is refactored to a pure DB read once corp/alliance names are denormalised onto `eve_character`.
- [x] 2c.5 Implement `backend/src/handlers/api/v1/characters.rs`:
  - `POST /api/v1/characters/:id/set-main` — verify the character belongs to the caller (404 otherwise); call `set_main` in a transaction; reload and return the updated character (same shape as one element of `GET /api/v1/me`'s `characters` array, including resolved corp/alliance names and `portrait_url`).
  - `DELETE /api/v1/characters/:id` — verify ownership (404 otherwise); if `is_main = true` and the account has >1 character → 409 `cannot_remove_main`; if it is the only character → 409 `cannot_remove_last_character`; otherwise hard-delete the row and return 204.
  - **NOTE:** the `set-main` reload's ESI fetch is removed by §2c.10 (same refactor as §2c.4).
- [x] 2c.6 Implement `backend/src/handlers/api/v1/account.rs`:
  - `DELETE /api/v1/account` — in a single Postgres transaction call `accounts::soft_delete(caller.account_id)`. After commit, delete every `session` row belonging to that account (via `SessionStore::list_session_ids_for_account` + `remove` in a loop, or a future `delete_for_account` helper). Set a session-cookie-clearing `Set-Cookie` header on the response. Return 204.
  - Extend the auth middleware (or the per-route guard) so that an `Authorization: Bearer erb_…` whose `account.status = 'soft_deleted'` is rejected with HTTP 401 and `error.code = "account_soft_deleted"` (per account-management spec).
- [x] 2c.7 Mount the new routes behind the `AuthenticatedAccount` middleware in `backend/src/main.rs` alongside `/api/v1/keys`.
- [x] 2c.8 Verify with `curl`: `GET /api/v1/me` returns the expected shape after login; `POST /api/v1/characters/<id>/set-main` flips `is_main`; `DELETE /api/v1/characters/<main_id>` returns 409 while siblings exist; `DELETE /api/v1/characters/<only_id>` returns 409; `DELETE /api/v1/account` returns 204 and the cookie is cleared. **Superseded by §2c.11** once the denormalisation refactor (§2c.10) lands, because the post-refactor handler has a different contract worth re-verifying.
- [x] 2c.9 **Add derived `token_status` to character responses** (added after §2c was originally implemented; required by the §4a-approved characters wireframe). In `backend/src/dto/`, extend the character DTO returned by `GET /api/v1/me` and by `POST /api/v1/characters/:id/set-main` with a `token_status: TokenStatus` field where `TokenStatus` is a serde-renamed string enum (`#[serde(rename_all = "snake_case")]`) with variants `Active` and `Expired`. The mapping rule lives in the service layer (NOT in the DTO `From` impl, since the DTO layer must not reach for `now()`): compute `token_status = if expires_at.map_or(false, |t| t > Utc::now()) { Active } else { Expired }` when projecting the `db::EveCharacter` row to a service struct, then derive the DTO from that. The raw `esi_token_expires_at` SHALL NOT appear in any response. Update the `utoipa::ToSchema` derive on the DTO and the `#[utoipa::path]` response annotations on both handlers so the OpenAPI doc lists `token_status` with its enum values. Extend `backend/tests/hurl/me.hurl` with one assertion that `data.characters[0].token_status` is present and is one of `"active"` or `"expired"`.

  **Integration-test coverage AND `derive_token_status` semantics deferred to §2c.10.** The task originally asked for two `#[sqlx::test]`-driven integration tests inserting character rows with `esi_token_expires_at` in the future and past and asserting the resulting `token_status`. That test shape needs `/api/v1/me` to be a pure DB read — the current handler's per-character `esi::public_info::fetch_*` calls would force any integration test that seeds a character row to make real ESI calls, which is unsuitable for CI. §2c.10 removes the ESI fetch from the handler; its sub-step 5 integration test SHALL assert both the corp/alliance names *and* `token_status` against the **revised** derivation rule (active when `encrypted_refresh_token IS NOT NULL`).

  Additionally, the `derive_token_status` rule shipped here (`expires_at > now()`) is wrong — see `design.md` §12's "Why `token_status` ignores `access_token_expires_at`" and the Risks/Trade-offs bullet. §2c.10 sub-step "Rewrite `derive_token_status`" replaces the rule with `has_refresh_token: bool` derived from `encrypted_refresh_token IS NULL`, rewrites the 4 unit tests added here, and renames `esi_token_expires_at` → `access_token_expires_at` in the schema and DB struct.

  **Done in this task:**
  - `TokenStatus` enum + `CharacterDto.token_status` field (`backend/src/dto/account.rs`).
  - `derive_token_status(Option<DateTime<Utc>>, DateTime<Utc>) -> TokenStatus` helper in `backend/src/services/account.rs`, called from `get_me` and `set_main_character` so both handlers carry the field through. **Rule changes in §2c.10.**
  - 4 unit tests for `derive_token_status` covering future / past / NULL / exact-boundary cases. **Replaced in §2c.10 to match the new rule.**
  - HURL assertion in `backend/tests/hurl/me.hurl` that `token_status` matches `^(active|expired)$` — still valid under the revised rule.
  - OpenAPI schema updates flow automatically via `utoipa::ToSchema` on `TokenStatus` and the existing `CharacterDto` annotation; `openapi_strict` integration tests pass against the regenerated schema.

- [x] 2c.10 **Denormalise corp/alliance names onto `eve_character`** (added after §2c.4–§2c.8 were originally implemented; required because `/api/v1/me` is called from the SvelteKit root `+layout.server.ts` on every authenticated page load — see §4.5 — and the original "fetch from ESI per request" rule fanned out into 2N serialised ESI calls per page load, dominating TTFB). All sub-steps are part of the same edit, sequenced so the build and tests stay green at the end:
  1. **Outbound HTTP tracing.** Add `reqwest-middleware` and `reqwest-tracing` to `backend/Cargo.toml` (pin to latest stable major). In the `AppState` construction site (wherever the shared `reqwest::Client` is built), replace `reqwest::Client` with a `reqwest_middleware::ClientWithMiddleware` wrapping the existing `Client` and a `TracingMiddleware`. Every outbound HTTP call (ESI and otherwise) then emits a `tracing` span at INFO/DEBUG level carrying method, URL host+path, status, and elapsed time. The shared client SHALL be threaded through the same places `&reqwest::Client` is threaded today; `esi::public_info` and `esi::discover` are the in-scope call sites. **This sub-step lands BEFORE the handler refactor below** so the curl re-verification in §2c.11 has a working "no ESI call should appear" signal. Document the dev-time grep: `RUST_LOG=erbridge=debug,reqwest_tracing=info` is enough to see one log line per outbound call.
  2. **Migration.** Edit `backend/migrations/00000000000001_create_account_eve_character_and_api_key.sql` in place (the change has not shipped, so a fix-up migration is unnecessary). On `eve_character`: add `corporation_name TEXT NOT NULL` after `corporation_id`; add `alliance_name TEXT` after `alliance_id`; **rename** `esi_token_expires_at` to `access_token_expires_at`; add `scopes TEXT[] NOT NULL DEFAULT '{}'` after the renamed column. Drop and recreate the local dev database (or `cargo sqlx database reset`) so the offline query cache regenerates against the new schema.
  3. **DB layer.** Update `backend/src/db/characters.rs`: the `Character` row struct gains `corporation_name: String`, `alliance_name: Option<String>`, and `scopes: Vec<String>`; renames `esi_token_expires_at` to `access_token_expires_at`. `upsert_tokens` and `create_orphan` signatures match §2.8 (the §2.8 task text was already updated to the new signatures): `upsert_tokens` gains `corporation_name: &str`, `alliance_name: Option<&str>`, `scopes: &[String]`, and renames the `expires_at` parameter to `access_token_expires_at`; `create_orphan` gains `corporation_name: &str` and `alliance_name: Option<&str>` (no `scopes` parameter — the column default `'{}'` handles orphans). `list_for_account` selects all the new columns. Regenerate `.sqlx/` via `cargo sqlx prepare -- --all-targets` after the queries compile.
  4. **SSO callback.** Update `backend/src/handlers/auth.rs` (`/auth/callback`):
     - Extend the `EsiJwtClaims` struct to deserialise the `scp` claim. EVE's JWT puts `scp` as either a single string (one scope granted) or an array of strings (multiple scopes), so deserialise via `#[serde(untagged)] enum Scp { One(String), Many(Vec<String>) }` and normalise to `Vec<String>` via a small `into_vec` method.
     - Call both `esi::public_info::fetch_corporation_name(corp_id)` and (when `alliance_id` is `Some`) `fetch_alliance_name(alliance_id)` after resolving IDs from the JWT and the corp public-info call. The two name fetches MAY run concurrently (`tokio::try_join!`) to avoid serialising two ESI round-trips on the auth hot path; the auth flow already accepts ESI latency since it only runs once per login.
     - Pass the resolved corp/alliance strings AND the normalised `scopes` vector into `upsert_tokens`.
  5. **`/api/v1/me` handler.** In `backend/src/handlers/api/v1/me.rs`, delete the per-character `esi::public_info::fetch_*` calls. Project `db::Character` directly to the response DTO; `corporation_name` comes from the row, `alliance_name` comes from the row's `Option<String>` (and is `null` in JSON when `None`). The handler SHALL no longer take the shared HTTP client. Add a `#[sqlx::test]`-driven Rust integration test that seeds three character rows on one account — one with a non-NULL `encrypted_refresh_token` ("active"), one with `encrypted_refresh_token = NULL` ("expired"), and one orphan-like row that is then claimed and given a refresh token (proves the projection works after upsert). The test asserts the JSON response carries the seeded `corporation_name` / `alliance_name` / `token_status` for each. **This integration test absorbs the deferred §2c.9 token_status integration tests.** The same test SHALL assert that the `tracing` subscriber captured zero spans whose URL host is `esi.evetech.net` (use `tracing_test::traced_test` or a small in-test subscriber) — this is the automated counterpart to the manual log-grep in §2c.11.
  6. **`/api/v1/characters/:id/set-main` handler.** In `backend/src/handlers/api/v1/characters.rs`, drop the ESI fetch from the post-update reload; read both names from the row returned by the DB. Same client-free signature.
  7. **Rewrite `derive_token_status`.** In `backend/src/services/account.rs`, change the helper signature from `derive_token_status(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> TokenStatus` to `derive_token_status(has_refresh_token: bool) -> TokenStatus`, returning `Active` when `has_refresh_token` and `Expired` otherwise. Update both call sites — `get_me` and `set_main_character` — to pass `c.encrypted_refresh_token.is_some()`. Drop the `now` capture (no `Utc::now()` call needed in this service layer anymore). **Replace the 4 unit tests added in §2c.9** with two new tests: one asserting `derive_token_status(true) == Active`, one asserting `derive_token_status(false) == Expired`. The boundary / future / past / NULL tests are no longer meaningful under the new rule. Keep the `portrait_url_format` test untouched.
  8. **DTO and OpenAPI annotations.** No DTO field changes (the response already exposes `corporation_name` / `alliance_name` / `token_status`); but if the DTO `From<CharacterInfo>` impl previously took a separate `&CorpInfo` parameter, simplify it to a single-input conversion. `#[utoipa::path]` response annotations stay correct (the wire shape is unchanged) but re-run `cargo test openapi_doc_matches_handler_responses` to confirm.
  9. **Tests.** Existing handler tests that mocked ESI for corp/alliance lookups SHALL be deleted; replace with DB-fixture-driven tests that seed `eve_character` rows with explicit names. Update `backend/tests/hurl/me.hurl` (and any sibling HURL files) to assert that `data.characters[0].corporation_name` equals the value seeded by the test fixture, not a value resolved at request time. The SSO-callback integration test SHALL assert that the row written by the callback contains `corporation_name`, (when applicable) `alliance_name`, AND a non-empty `scopes` array matching the scopes from the mocked JWT.
  10. **Skill-layout check.** `rust-rest-api` skill is authoritative for backend module layout (per `CLAUDE.md`). The `esi::public_info` module stays — both name functions remain on it for use by the SSO callback now, and by the future SSE-driven background job that refreshes corp/alliance names for active accounts on a schedule. Do NOT inline the fetches into `handlers/auth.rs`.

- [x] 2c.11 **Re-verify §2c with the refactored handlers** (supersedes §2c.8). Boot the backend with `RUST_LOG=erbridge=debug,reqwest_tracing=info` so every outbound HTTP call logs one line. Then:

  **Verified 2026-05-22** against the production compose stack (running at `RUST_LOG=erbridge=info,tower_http=info` — INFO not DEBUG, which is sufficient because the `reqwest-middleware` tracing layer logs outbound calls at INFO; the spec asked for `debug` out of caution). The smoking-gun (sub-step 3) was the load-bearing check: I issued `UPDATE eve_character SET corporation_name = 'PROOF-OF-DB-READ' WHERE id = '<main>'`, then `curl /api/v1/me` and the response carried `corporation_name: "PROOF-OF-DB-READ"` with zero `esi.evetech.net` log lines emitted for the duration of the request. Sub-step 5 (the 409 paths) was discharged via §7.20. Sub-step 4 (`set-main` reload from DB) was exercised by §7.19 in the browser without any ESI traffic in the log. Sub-step 1 (ESI lines appear during the SSO callback) is implicit in §7.4 having succeeded — the callback completed end-to-end, which it cannot do without contacting the token endpoint and public-info endpoints. The integration test `get_me_returns_db_fields_and_token_status` (backend/tests/me.rs) is the automated counterpart: it builds the router with a real DB and asserts the response shape comes from seeded rows, passing without network access — the strongest evidence that the handler never reaches for ESI.

  1. Complete an SSO login. Confirm the backend logs show `esi.evetech.net` lines during `/auth/callback` (token endpoint + corp/alliance public-info) — this proves the tracing is wired and ESI traffic is observable.
  2. `curl --cookie …` `GET /api/v1/me`. Confirm the response shape matches `account-management/spec.md` with `corporation_name` and `alliance_name` populated. **Confirm zero `esi.evetech.net` lines appear in the backend log for the duration of this request.**
  3. `UPDATE eve_character SET corporation_name = 'PROOF-OF-DB-READ' WHERE id = '…'` directly in psql, then re-`curl` `/api/v1/me` and confirm the response reflects `PROOF-OF-DB-READ` without a backend restart and with zero new `esi.evetech.net` log lines. This is the smoking-gun test that the handler is a pure DB read.
  4. `POST /api/v1/characters/<id>/set-main` flips `is_main` and the response's `corporation_name` / `alliance_name` come from the DB (same `'PROOF-OF-DB-READ'` trick if you want to be sure); zero ESI log lines.
  5. `DELETE /api/v1/characters/<main_id>` returns 409 while siblings exist; `DELETE /api/v1/characters/<only_id>` returns 409; `DELETE /api/v1/account` returns 204 and the cookie is cleared. (Same as §2c.8 — these paths never called ESI; this re-runs them as a regression check.)

## 2d. Backend: OpenAPI document via `utoipa` (strict)

These tasks discharge the api-contract spec's "Machine-readable API description" requirement. They depend on §2b and §2c being implemented first (the handlers and DTOs are what get annotated).

- [x] 2d.1 Add `utoipa` and `utoipa-swagger-ui` (with the `axum` feature on `utoipa-swagger-ui`) to `backend/Cargo.toml`. Pin to the latest stable major.
- [x] 2d.2 Derive `utoipa::ToSchema` on every request/response DTO in `backend/src/dto/` and on the success-envelope (`ApiResponse<T>`) and error-envelope (`ApiError`) types. The envelope types SHALL be declared once and referenced from every annotated response, not inlined per-route.
- [x] 2d.3 Annotate every `/api/v1/*` handler with `#[utoipa::path(...)]`:
  - `POST /api/v1/keys`, `GET /api/v1/keys`, `DELETE /api/v1/keys/:id` (from §2b)
  - `GET /api/v1/me`, `POST /api/v1/characters/:id/set-main`, `DELETE /api/v1/characters/:id`, `DELETE /api/v1/account` (from §2c)
  - Each annotation SHALL declare: HTTP method, path, request body schema (where applicable), one response per status code returned (including 2xx, 4xx, and 5xx envelopes), `security` requirements (session cookie or bearer), and the canonical `error.code` values it may return as part of the 4xx response descriptions.
- [x] 2d.4 Create `backend/src/openapi.rs`: a struct with `#[derive(utoipa::OpenApi)]` listing every annotated path and every component schema (DTOs + envelopes). Set `info.title = "E-R Bridge API"`, `info.version` from `CARGO_PKG_VERSION`.
- [x] 2d.5 Mount routes in `backend/src/main.rs`:
  - `GET /api/openapi.json` — returns `ApiDoc::openapi().to_json()?` with `Content-Type: application/json`.
  - `GET /api/docs` — Swagger UI bound to `/api/openapi.json`, via `utoipa_swagger_ui::SwaggerUi`.
  - Both routes SHALL be public (no auth) so external clients and the Swagger UI can fetch them. They live outside the `AuthenticatedAccount` middleware tree.
- [x] 2d.6 Write `backend/tests/openapi_strict.rs`: an integration test that asserts every documented route's actual response validates against its declared schema. Concrete implementation:

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

- [x] 2d.7 Add a doc-coverage test in the same file:

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
- [x] 2d.8 Verify with `curl`: `curl $APP_URL/api/openapi.json | jq .openapi` returns `"3.1.0"` (or the version `utoipa` emits — note in the test if it's `3.0.x`); `curl $APP_URL/api/docs` returns HTML with `<title>Swagger UI</title>`.

## 3. Backend: Dockerfile

- [x] 3.1 Write `backend/Dockerfile`: multi-stage build — `rust:latest` builder stage compiling release binary, then `debian:bookworm-slim` runtime stage copying binary
- [x] 3.2 Ensure `EXPOSE 3000` (or chosen internal port) and `CMD` are set correctly

## 4a. Wireframes (author and approve BEFORE frontend implementation)

These wireframes are the authoritative visual contract for section 4 (per design.md §10). Each file SHALL be a standalone, self-contained HTML page that can be opened in a browser with no build step. They SHALL inline the design tokens from design.md §10 (`--space-*`, slate scale, `--sky`, `--emerald`, `--red`) on `:root` and load JetBrains Mono from Google Fonts, so the rendered HTML is visually accurate to within ~5% of the final Svelte implementation. Use realistic placeholder data ("Wasp 223", "Artemisia de'Halicarnass", "Exit-Strategy", "Unchained Alliance") so the layout is reviewable.

The user reviews and approves these wireframes before any task in section 4 begins. Tweaks in this phase are cheap; tweaks after Svelte implementation are not.

- [x] 4a.1 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/login.html` matching screenshot 01: centred card on `--space-950` background, cyan sun logo, `E-R BRIDGE` wordmark, "Wormhole Mapper" subtitle, divider, official EVE SSO PNG button (`https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png`), two-line disclaimer in `--slate-500`.
- [x] 4a.2 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/home.html` matching screenshot 02: 48px GlobalNav at top with logo + brand + `maps` + `characters` links + pulsing emerald `connected` dot + user chip (24px portrait + name + chevron); centred-left body content reading `Welcome, Wasp 223` in `--sky`, then the main character name + `main` badge + corporation/alliance lines. No "Map view coming soon." line — that placeholder lives on `/maps`.
- [x] 4a.3 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/maps.html`: same GlobalNav as `home.html` with `maps` shown as the active link; centred body with a single line `Map view coming soon.` in `--slate-500`. The wireframe SHALL also include one **layout-level error banner** example per design.md §14: a strip in `--red` directly beneath the GlobalNav reading "Couldn't load your account: <error.message>", so the banner's visual treatment is locked in. The maps page is a fine host for this example since the other pages will rely on the same shared component. This is the placeholder for the future map-rendering change.
- [x] 4a.4 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/characters.html`: same GlobalNav with `characters` shown as the active link; page heading `CHARACTERS` in `--slate-500` uppercase, plus a search box and a `+ add character` button (`href="/auth/characters/add?return_to=/characters"`) in the header. Character cards are laid out in a **2-column responsive grid**. The wireframe SHALL include at least six cards covering: the main character (no `set main` / `remove`), at least two non-main characters with `token_status = "active"` (with `set main` + `remove` actions), and at least two non-main characters with `token_status = "expired"` (with `re-auth` in `--amber` linking to `/auth/characters/add?return_to=/characters`, in place of `set main`, alongside `remove`). Each card has a card-footer with a token-status indicator (small dot + `token active` in `--emerald` or `token expired` in `--red`) on the left and actions on the right. The search input SHALL be wired up with a small `<script>` block: typing filters cards client-side by case-insensitive substring match on `data-name`; an empty-state message ("No characters match your search.") is shown when no card matches. Below the grid, a horizontal divider and a `DANGER ZONE` section with `delete account` text button in `--red`. The wireframe SHALL also include one example **error state** per design.md §14: re-render a non-main card with a `--red` inline error message ("Couldn't remove character: cannot_remove_main") anchored directly under it, so the visual treatment of inline form-action errors is locked in.
- [x] 4a.5 Author `openspec/changes/eve-wormhole-mapper-foundation/wireframes/user-menu.html` matching screenshot 06: render `home.html`'s top-right chrome with the user-menu dropdown open beneath the user chip. Items: `preferences` (greyed-out, `aria-disabled="true"`), `settings` (greyed-out, `aria-disabled="true"`), divider, `log out` (`href="/auth/logout"`). Card surface is `--space-900` with `--space-700` border, anchored to the chip's right edge.
- [x] 4a.6 The user opens each wireframe in a browser and signs off. If anything looks wrong (spacing, copy, palette, hover states, dropdown anchoring, badge style), the wireframe is updated before section 4 begins. The implementer SHALL NOT proceed to section 4 with un-approved wireframes.

  **Spec-sync rule**: design.md §11 ("Visible UI surface") and §14 ("UI surface for API errors") describe load-bearing behaviour (which character drives the user chip, what `maps` resolves to, where errors surface). If wireframe review changes any of those decisions — e.g. you decide errors should be toasts after all, or `maps` should be `/` not `/maps`, or `delete account` should require a confirmation step — the implementer SHALL update the relevant section of design.md *and* re-run `openspec validate eve-wormhole-mapper-foundation` before §4 begins. The wireframes and design.md MUST agree at approval time; downstream tasks (and the rust-rest-api / sveltekit-node skills) read both, so they can't drift.

  Cosmetic-only changes (spacing, font weight, exact shade of red, hover transitions, badge corner radius) do NOT require a design.md update — they live entirely in the wireframe.

## 4. Frontend: SvelteKit Project

All tasks in this section are blocked on §4a approval. Each task SHALL produce output that matches the corresponding wireframe; if a wireframe is silent on something (e.g. exact hover colour), it falls back to design.md §10/§11.

- [x] 4.1 Scaffold SvelteKit app in `frontend/` using `npm create svelte@latest` with Svelte 5 and TypeScript
- [x] 4.2 Install `@sveltejs/adapter-node`; update `svelte.config.js` to use it
- [x] 4.3 Create `frontend/src/app.css`: import JetBrains Mono from Google Fonts; define all design-system CSS custom properties (`--space-950` through `--space-600`, full slate scale, `--sky`, `--emerald`, `--amber`, `--red`) on `:root` — names and values MUST match the wireframes' inlined tokens exactly so the Svelte build is pixel-equivalent. Set the `html` element to `font-size: 100%` so it picks up the browser/OS default (typically 16px), then set `body` to `margin: 0; padding: 0; background: var(--space-950); color: var(--slate-100); font-family: "JetBrains Mono", ui-monospace, monospace; font-size: 0.875rem; line-height: 1.5`. **All typography rules in this file and every Svelte component MUST use `rem` (not `px`) so the UI scales with user/browser font-size preferences**; spacing (padding, margin, gap, border-radius, avatar/icon `width`/`height`, border widths) stays in `px`. This `rem`-everywhere rule is the precondition for the user-controllable text-size preference planned in the `accessibility-preferences` change. Import in `+layout.svelte`.
- [x] 4.4 Create `frontend/src/lib/api.ts`: typed wrapper around the backend's `/api/v1/me`, `/api/v1/characters/:id/set-main`, `/api/v1/characters/:id`, and `/api/v1/account` endpoints. Each function returns the unwrapped `data` payload on success and throws a typed error for non-2xx responses (carrying `error.code` and `error.message` from the envelope). Types are hand-typed in this change to mirror the backend OpenAPI doc at `/api/openapi.json`; a future change wires in `openapi-typescript` (or similar) to generate them. Every type in this file SHALL include a `// keep in sync with: backend/src/dto/<file>.rs` comment pointing at the backend source.
- [x] 4.4a Declare `App.Locals` in `frontend/src/app.d.ts`: add `interface Locals { me: MeResponse | null }` so `event.locals.me` is typed end-to-end in `+layout.server.ts`, `+page.server.ts`, and form actions. `MeResponse` is imported from `frontend/src/lib/api.ts`.
- [x] 4.5 Implement `frontend/src/routes/+layout.server.ts`: on every request, call backend `GET /api/v1/me` (forwarding the `cookie` header). On 401, set `event.locals.me = null` and redirect to `/login` unless the current route is `/login`. On 200, store the response in `event.locals.me`. If the request targets `/login` while `event.locals.me` is set, redirect to `/`.

  **Per-request cost.** `GET /api/v1/me` is contractually a pure DB read (per `account-management/spec.md` and §2c.10): corp and alliance names are denormalised onto `eve_character` and refreshed at SSO callback time (and by a future background job), so this handler does no ESI traffic. The layout therefore costs one round-trip to the backend plus one indexed Postgres `SELECT` per page load — that is the budget §4.5 was designed against, and the design will fall apart if the handler reverts to per-request ESI fan-out. If you are modifying the handler and find yourself adding an outbound HTTPS call, stop and re-read §12 of `design.md`.

  **Lifecycle**: `event.locals.me` is populated *only* by this layout load, and only for the duration of a single SvelteKit request. It is NOT a long-lived store, NOT a session, and is NEVER set by client-side code or by hook code outside this layout. After the SSO callback (`/auth/callback` on the backend, not in SvelteKit) redirects the browser to `/` or to `return_to`, that subsequent request goes through this layout again and `locals.me` is repopulated from the now-valid session cookie. There is no path by which `locals.me` carries state across requests; each request fetches `/api/v1/me` afresh. This keeps the auth gate trivial: the cookie is the only durable state, and the layout is the single point that reads it.

  **Errors**: 401 → redirect to `/login` (no banner). Network error or 5xx → set `locals.me = null` AND return `{ me: null, meError: { code, message } }` so the layout can render the top-of-page error banner per design.md §14. The page still renders (degraded). 4xx other than 401 → same as 5xx (treated as recoverable, banner shown).
- [x] 4.6 Create `frontend/src/lib/components/GlobalNav.svelte` matching `wireframes/home.html`'s top bar exactly: 48px bar (`background: var(--space-900)`, `border-bottom: 1px solid var(--space-700)`); brand logo SVG in `--sky` + `E-R BRIDGE` wordmark linking to `/`; nav links `maps` (→ `/maps`) and `characters` (→ `/characters`), active link in `--sky` with a `--space-700` pill; right side: pulsing `--emerald` status dot + `connected` label (red + `disconnected` if `me` load failed); to its right, the `UserChip` component (4.7).
- [x] 4.7 Create `frontend/src/lib/components/UserChip.svelte`: 24px circular portrait of the **main** character + main character name + chevron caret. Click toggles a `UserMenu` (4.8). Closes on outside-click and `Escape`.
- [x] 4.8 Create `frontend/src/lib/components/UserMenu.svelte` matching `wireframes/user-menu.html`: dropdown card (`--space-900` background, `--space-700` border, anchored to the chip's right edge); items `preferences` (`aria-disabled="true"`, `tabindex="-1"`, no hover), `settings` (same), `--space-700` divider, `log out` (`<a href="/auth/logout">`). Leave a one-line breadcrumb comment immediately above the `preferences` item: `<!-- TODO(accessibility-preferences): wire this up — see openspec/changes/accessibility-preferences/ -->`. This makes the placeholder discoverable to whoever opens the file next, in case the `accessibility-preferences` stub change has not yet been actioned.
- [x] 4.9 Create `frontend/src/routes/+layout.svelte`: full-height shell (`display: flex; flex-direction: column; height: 100vh; overflow: hidden; background: var(--space-950)`); include `GlobalNav` at top **unless** the current route is `/login` (login has no nav per wireframe); when `data.meError` is set (the `me` fetch failed with non-401), render the layout-level error banner described in design.md §14 directly below the GlobalNav; yield `{@render children()}` for main content.
- [x] 4.10 Implement `frontend/src/routes/login/+page.svelte` matching `wireframes/login.html`: full-viewport `--space-950` background; centred card (`background: var(--space-900)`, `border: 1px solid var(--space-700)`, `border-radius: 8px`); E-R Bridge logo SVG in `--sky`; wordmark + "Wormhole Mapper" subtitle; `<hr>` divider; `<a href="/auth/login"><img src="https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png" alt="LOG IN with EVE Online"></a>`; two-line disclaimer in `--slate-500`.
- [x] 4.11 Implement `frontend/src/routes/+page.server.ts`: re-use `event.locals.me` from the layout (no separate fetch); pass `me` to the page as `data.me`.
- [x] 4.12 Implement `frontend/src/routes/+page.svelte` (Svelte 5 syntax) matching `wireframes/home.html`: centred-left content area; `Welcome, <main.name>` heading in `--sky`; main character name + `main` badge + corporation row + alliance row (alliance row omitted when `alliance_id IS NULL`). No sidebar. No canvas. No "Map view coming soon." line — that placeholder lives on `/maps`. The sidebar from the original design is deferred to the future map-rendering change.
- [x] 4.13 Implement `frontend/src/routes/maps/+page.svelte` matching `wireframes/maps.html`: centred body with a single line `Map view coming soon.` in `--slate-500`. No `+page.server.ts` needed; `event.locals.me` from the layout is enough to gate access. This is the placeholder destination for the `maps` nav link.
- [x] 4.14 Implement `frontend/src/routes/characters/+page.server.ts`: re-use `event.locals.me`; return `{ characters: locals.me.characters }`. Also expose form actions, each of which returns `fail(status, { code, message })` on the envelope's `error.code` / `error.message` so the page can surface them inline per design.md §14:
  - `setMain` — `POST /api/v1/characters/:id/set-main` then invalidate the layout load
  - `remove` — `DELETE /api/v1/characters/:id` then invalidate; surface `error.code` (`cannot_remove_main`, `cannot_remove_last_character`) as a user-visible message anchored to the character's card
  - `deleteAccount` — `DELETE /api/v1/account` then `redirect(303, '/login')` on success; on failure, surface the error inline under the DANGER ZONE button
- [x] 4.15 Implement `frontend/src/routes/characters/+page.svelte` matching `wireframes/characters.html`: page heading `CHARACTERS` + search box + `+ add character` link (`href="/auth/characters/add?return_to=/characters"`) in the header. Character cards rendered from `data.characters` into a **2-column responsive grid** (1 column on narrow viewports): portrait from `portrait_url`, name, `main` badge for the main, corporation row, alliance row when present, and a card-footer with a token-status indicator (dot + `token active` in `--emerald` when `character.token_status === "active"` or `token expired` in `--red` when `"expired"`) on the left and action buttons on the right. Non-main cards with `token_status === "active"` show `set main` + `remove` (form actions from 4.14); non-main cards with `token_status === "expired"` show `re-auth` (a plain `<a href="/auth/characters/add?return_to=/characters">` styled in `--amber`) in place of `set main`, alongside `remove`. The main card has no `set main` / `remove` actions but still shows its token-status indicator and a `re-auth` link when expired. The search input filters the grid client-side using a `$state`-backed query with case-insensitive substring match against `character.name` (use `$derived` to build the filtered list); when the filtered list is empty, render an inline empty state ("No characters match your search."). Inline error rendered in `--red` directly below a card when its action's `form.code` is set, with `data-error-code={form.code}` attached so tests can branch on the code. Divider; `DANGER ZONE` section with `delete account` form action button and an inline error slot underneath it. Errors surface inline per design.md §14; no toasts and no `+error.svelte` route.
- [x] 4.16 Verify `npm run build` produces a `build/` directory with a runnable Node.js server.
- [x] 4.17 Open each implemented page side-by-side with its wireframe in a browser; the layouts SHALL be visually equivalent. Tune spacing, colours, and hover states until they match. Document any deliberate deviations as a comment in the relevant `+page.svelte`.

## 5. Frontend: Dockerfile

- [x] 5.1 Write `frontend/Dockerfile`: multi-stage build — `node:lts` builder stage running `npm ci && npm run build`, then slim runtime stage running `node build`
- [x] 5.2 Ensure `EXPOSE 3000` (or chosen port) and `CMD ["node", "build"]` are set

## 6. Traefik + Postgres Configuration

- [x] 6.1 Write `traefik.yml` (static config): enable Docker provider, set entrypoint on port 80, disable dashboard in production
- [x] 6.2 Write `docker-compose.yml` with four services:
  - `traefik`: mounts `/var/run/docker.sock` and `traefik.yml`; publishes port 80
  - `postgres`: `postgres:18` image (bumped from the task-spec's `postgres:16` — `:18` is what the dev compose has been running on and what migrations were verified against); env vars `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` from `.env`; named volume `postgres-data:/var/lib/postgresql/data`; healthcheck using `pg_isready`
  - `backend`: built from `./backend`; env vars from `.env` including `DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}`; `depends_on: postgres: condition: service_healthy`; Traefik labels routing `/auth/` and `/api/` to it
  - `frontend`: built from `./frontend`; env vars `BACKEND_INTERNAL_URL=http://backend:3000` and `ORIGIN=${APP_URL}`; Traefik label routing all other requests to it
- [x] 6.3 Declare the `postgres-data` named volume at the top level of `docker-compose.yml`
- [x] 6.4 Confirm routing rules use `PathPrefix` matchers and that backend rule has higher priority than frontend catch-all

## 7. Integration Verification

- [x] 7.1 Copy `.env.example` to `.env`, fill in real EVE SSO credentials and generated `ENCRYPTION_SECRET`
- [x] 7.2 Run `docker compose up --build`; confirm all four containers reach running state and backend logs report migrations applied
- [x] 7.3 Navigate to `APP_URL/`; confirm redirect to `/login`
- [x] 7.4 Click the EVE SSO button; complete the SSO flow; confirm redirect to `/` shows the character name
- [x] 7.5 Connect to Postgres (`docker compose exec postgres psql -U $POSTGRES_USER $POSTGRES_DB`); confirm one row in `account` (`status = 'active'`, `is_server_admin = false`) and one row in `eve_character` with non-NULL `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `corporation_id`, `corporation_name`, and a non-empty `scopes` array (and, when the character's corp is in an alliance, non-NULL `alliance_id` and `alliance_name`)
- [x] 7.6 Visit `/auth/characters/add`; complete SSO with a second EVE character; confirm a second row in `eve_character` linked to the same `account_id`
- [x] 7.7 Run `docker compose down && docker compose up`; log in again with the same character; confirm the existing `eve_character` row is updated (same `id`, new `updated_at`, new ciphertext bytes due to fresh nonce) rather than duplicated
- [x] 7.8 Soft-delete sanity: temporarily issue `UPDATE account SET status='soft_deleted', delete_requested_at = now()`; log in again; confirm `status` returns to `'active'` and `delete_requested_at` is NULL
- [x] 7.9 Visit `APP_URL/auth/logout`; confirm redirect to `/login`
- [x] 7.10 Verify session cookie is `httpOnly` and `SameSite=Lax` in browser devtools
- [x] 7.11 Verify no token material (access or refresh) appears in any cookie or response header, and that plaintext tokens never appear in backend logs
- [x] 7.12 API key end-to-end: while authenticated by session cookie, `curl -X POST $APP_URL/api/v1/keys -d '{"name":"smoke","expires_at":null}'`; capture returned plaintext `key`; confirm it matches `erb_[A-Za-z0-9_-]{43}`; confirm one row in `api_key` with `scope = 'account'`, matching `account_id`, and `key_hash` equal to `echo -n "$KEY" | sha256sum | cut -d' ' -f1`
- [x] 7.13 API key authenticates: `curl -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys`; confirm the response lists the key and does NOT include a plaintext field
- [x] 7.14 API key revocation: `curl -X DELETE -H "Authorization: Bearer $KEY" $APP_URL/api/v1/keys/$ID`; confirm 204; repeat the previous `curl` with the same key and confirm 401
- [x] 7.15 Visit `/login` in a browser: layout matches `wireframes/login.html` (logo, wordmark, subtitle, EVE SSO button, disclaimer). The page has no GlobalNav.
- [x] 7.16 After SSO, the browser lands on `/` and the layout matches `wireframes/home.html`: GlobalNav with brand + `maps` + `characters` + emerald `connected` dot + user chip (main character's portrait + name + chevron); body shows `Welcome, <main name>` plus the main character block plus `Map view coming soon.`
- [x] 7.17 Click the user chip: dropdown appears matching `wireframes/user-menu.html` (`preferences` and `settings` greyed-out and non-interactive; divider; `log out`). Click outside or press `Escape`: dropdown closes. Click `log out`: redirected to `/login`.
- [x] 7.18 Click `maps` in the nav: layout matches `wireframes/maps.html` (GlobalNav with `maps` active, body shows `Map view coming soon.`). Click `characters` in the nav: layout matches `wireframes/characters.html` with `characters` active. Character cards are laid out in a **2-column grid**. The main character's card has no `set main` / `remove` actions; non-main cards with an active token have `set main` and `remove`; non-main cards with an expired token have `re-auth` (linking to `/auth/characters/add?return_to=/characters`) in place of `set main`, alongside `remove`. Each card displays a token-status dot in `--emerald` (`token active`) or `--red` (`token expired`). Click `+ add character`: redirected to EVE SSO; after completing with a third character, the new row appears in the list **and the browser returns to `/characters`** (not `/`) because the `?return_to=/characters` hint was honoured. The previous main is unchanged.
- [x] 7.19 Click `set main` on a non-main character: the page reloads, the badge moves, and the user chip in the GlobalNav now shows the new main's portrait and name.
- [x] 7.19a In Postgres, NULL out one non-main character's refresh token (`UPDATE eve_character SET encrypted_refresh_token = NULL WHERE id = ...`); reload `/characters` and confirm that character's card now shows a red `token expired` dot, its `set main` action is replaced by an amber `re-auth` link pointing at `/auth/characters/add?return_to=/characters`, and `remove` is still available. `GET /api/v1/me` SHALL return `token_status = "expired"` for that character. Click `re-auth`, complete SSO with the same EVE character; on return to `/characters`, the dot is back to green, the `set main` action returns, and Postgres shows the row's `encrypted_refresh_token` is non-NULL again.
- [x] 7.19b On `/characters`, type a partial character name into the search box. Confirm only matching cards remain visible (case-insensitive substring match on character name). Clear the search; all cards return. Type a string that matches no character; confirm the empty-state message ("No characters match your search.") is shown.
- [x] 7.20 Attempt to `remove` the main character via direct API call (`curl -X DELETE`): confirm 409 `cannot_remove_main`. With only one character linked, attempt to remove it: confirm 409 `cannot_remove_last_character`.
- [x] 7.21 Click `delete account` in the DANGER ZONE: response is 204, session cookie is cleared, browser redirects to `/login`. Confirm in Postgres that `account.status = 'soft_deleted'` and `delete_requested_at` is set; `eve_character` rows are untouched.
- [x] 7.22 Log in again as the soft-deleted account's character: per existing 7.8, the account is reactivated. Visit `/`: the home page renders normally.
- [x] 7.23 With a soft-deleted account (before re-login), attempt to use a previously-issued API key: confirm 401 with `error.code = "account_soft_deleted"`.
- [x] 7.24 Open the rendered pages side-by-side with their wireframes. Any deviation that is not documented as deliberate is treated as a bug and fixed before this change is considered complete.
- [x] 7.25 `GET /api/openapi.json` returns HTTP 200 with `Content-Type: application/json` and a body whose `openapi` field is `"3.1.0"` (or the version `utoipa` emits — record it in the test). The document includes every `/api/v1/*` route mounted by the backend.
- [x] 7.26 `GET /api/docs` renders a Swagger UI page (`<title>Swagger UI</title>` in the HTML; the JSON model URL points at `/api/openapi.json`).
- [x] 7.27 The backend `cargo test` suite passes including the strict-drift test (§2d.6) and the doc-coverage test (§2d.7). To prove the drift test bites, temporarily change one handler's response shape without updating its annotation and confirm the test fails; revert. **Verified 2026-05-22:** 95/95 tests pass. Perturbation: added `#[serde(serialize_with = "...")]` to `AccountDto.is_server_admin` to serialize the bool as a string. `get_me_200_matches_schema` failed with `ValidationError { instance: String("false"), kind: Type { kind: Single(Boolean) }, instance_path: ... is_server_admin }`. Reverted; all tests pass again.
- [x] 7.28 `GET /auth/login?return_to=/characters` followed by SSO completion redirects the browser to `/characters` (not `/`). Repeat with `?return_to=https://evil.example.com/` and confirm the callback redirects to `/` (off-origin rejected). Repeat with `?return_to=//evil.example.com/` and confirm the callback redirects to `/` (scheme-relative rejected).
- [x] 7.29 **Pre-archival**: move `openspec/changes/eve-wormhole-mapper-foundation/wireframes/` to `frontend/wireframes/` so the wireframes survive archival as a tracked, durable artefact. Update any references in the archived design.md note to point at the new location. Delete `zz-ref/frontend/screenshots/` — the wireframes have superseded them. Run this task only when all earlier §7 checks have passed and the change is ready to be archived.
