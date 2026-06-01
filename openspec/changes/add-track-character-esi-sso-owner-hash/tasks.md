## 1. Schema

- [x] 1.1 Add migration `00000000000008_*` adding to `eve_character`: `owner_hash TEXT` (nullable) and `token_status TEXT NOT NULL DEFAULT 'valid'` with `CHECK (token_status IN ('valid','token_expired','owner_mismatch'))`; and to `account`: `last_login TIMESTAMPTZ` (nullable).
- [x] 1.2 Run the migration against the local dev/test DB so sqlx compile-time checks see the new columns.

## 2. ESI token refresh (`backend/src/esi/token.rs`)

- [x] 2.1 Extend `RefreshedTokens` with the `owner` hash, parsed from the refreshed access-token JWT (reuse the same JWT-claim parsing as the callback; factor a shared helper if cleaner per the `rust-rest-api` skill).
- [x] 2.2 Unit test: a refreshed access token's `owner` claim is surfaced on `RefreshedTokens`.

## 3. DB layer (`backend/src/db/`)

- [x] 3.1 `db/characters.rs`: add `owner_hash` and `token_status` to the `Character` struct and every SELECT / row mapping that returns a `Character`.
- [x] 3.2 Thread `owner_hash` and `token_status = 'valid'` through `upsert_tokens` and `create_orphan` (and any other write path), so callback writes record the hash and reset state to valid.
- [x] 3.3 Add a function to set a character's `token_status` and NULL its credential columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, empty `scopes`) by `eve_character_id`. *(Implemented pool-based, not tx-scoped: the sweep runs outside any request transaction, so `set_token_status` / `expire_valid_tokens_for_account` take `&PgPool`. The mismatch audit write uses its own short tx, matching the `BlockedLoginRejected` pattern.)*
- [x] 3.4 Add a query returning characters eligible for the sweep: `token_status <> 'token_expired'` and holding a refresh token (returns id/eve_character_id, encrypted refresh token, stored `owner_hash`).
- [x] 3.5 Extend `update_tokens_by_eve_id` (the refresh writer) to also store the observed `owner_hash` and set `token_status = 'valid'` on a matching-hash success.
- [x] 3.6 `db/accounts.rs`: read/write `account.last_login`; add a query for accounts whose `last_login < now() - interval '7 days'` (treat NULL as not-yet-idle, i.e. excluded, to avoid mass-expiring legacy rows on first run).
- [x] 3.7 Unit tests (`#[sqlx::test]`, one per function): owner_hash + token_status round-trip through upsert/orphan/select; set-status NULLs credentials; the eligibility query excludes `token_expired` and tokenless rows; the idle query selects only accounts past the 7-day threshold and excludes NULL `last_login`.

## 4. Audit event (`backend/src/audit/mod.rs`)

- [x] 4.1 Add a dormant variant `CharacterOwnerMismatch { eve_character_id, account_id }` (kind string e.g. `character_owner_mismatch`) following the existing dormant-variant house style and target mapping.
- [x] 4.2 Unit test: the variant serialises with the expected kind string and payload fields.

## 5. Background sweep (new module per `rust-rest-api` skill layout)

- [x] 5.1 Add a sweep module exposing one run-once entry point (so it is unit-testable without a timer): iterate eligible characters (task 3.4); for each, refresh (task 2.x) → on success+matching-hash store rotated tokens and keep `valid`; on success+differing-hash set `owner_mismatch`, NULL credentials, record hash, emit audit (task 4); on failure set `token_expired`, NULL credentials.
- [x] 5.2 In the same run, apply the 7-day idle waterfall (task 3.6): expire still-`valid` characters of idle accounts.
- [x] 5.3 Spawn the sweep on an ~24h interval at startup in `main.rs` (the first background task in the codebase) — interval loop calling the run-once entry point; log run summaries; one run's failure must not kill the task.
- [x] 5.4 Consider throttling/batching of refresh calls; keep configurable or sensibly bounded.
- [x] 5.5 Unit tests against the run-once entry point (mock db + refresh): matching-hash keeps valid; differing-hash → owner_mismatch + audit + NULLed credentials; refresh failure → token_expired; already-`token_expired` rows are skipped; idle-account characters are expired.

## 6. Callback wiring (`backend/src/handlers/auth.rs`, `backend/src/services/auth.rs`)

- [x] 6.1 Add `owner: String` to `EsiJwtClaims` (always present on ESI access tokens; treat absence as a `BadGateway` parse error, consistent with `sub`). Thread it into the callback service input.
- [x] 6.2 In `services/auth.rs`, persist `owner_hash`, set `token_status = 'valid'`, and set `account.last_login = now()` on every successful callback. No owner-hash *change* detection in the callback (the sweep owns that).
- [x] 6.3 Unit tests: `parse_esi_jwt_claims` extracts `owner` and rejects a JWT missing it; the callback records owner_hash + last_login and resets token_status to valid (covers self-heal of a previously-flagged row on matching hash).

## 7. Admin API — character search + token state (`backend/src/handlers/`, `services/admin.rs`, `db/`)

- [x] 7.1 Add/extend an admin endpoint to search characters by name and return, for a selected character, its whole account with every character and each character's `token_status`. Reuse the existing admin auth (`AdminAccount` extractor) and the existing account-with-characters assembly where possible.
- [x] 7.2 Expose `token_status` on the relevant admin DTOs and on the user-facing `GET /api/v1/me` character shape.
- [x] 7.3 Tests: search returns matching characters; the account view includes all characters with token_status; admin-only access enforced.

## 8. Frontend — token state + admin Characters tab (`frontend/`, per `sveltekit-node` skill)

- [x] 8.1 Render `token_status` on the user's own character list: `token_expired` → a "reconnect"/re-login affordance; `owner_mismatch` → a "no longer on your account / remove" affordance. (Copy can be iterated.)
- [x] 8.2 Add an admin **Characters** tab: search a character by name; selecting a result opens a dialog showing the whole account and all characters with their `token_status`.
- [x] 8.3 Support surfacing/sorting/filtering the character listing by token state (at minimum, find `token_expired` and `owner_mismatch` characters together).
- [x] 8.4 Add i18n message keys for the new states/affordances across all locale files (en/de/fr), keeping the locale set in sync (run paraglide from `frontend/`).
- [x] 8.5 Component/unit tests (Vitest) for the status rendering and the admin search/dialog; e2e (Playwright) for the admin Characters tab search→dialog flow.

## 9. Tooling & verification

- [x] 9.1 Regenerate the sqlx offline cache from `backend/`: `cargo sqlx prepare -- --all-targets`; commit the `.sqlx/` diff.
- [x] 9.2 `cargo fmt` and `cargo clippy --all-targets` clean.
- [x] 9.3 `cargo test` (backend unit + integration) passes.
- [x] 9.4 `cargo sqlx prepare --check -- --all-targets` passes (no cache drift).
- [x] 9.5 Ran the HURL suite against the live dev stack: `admin.hurl` 29/29, `me.hurl` 2/2, `keys.hurl` 8/8, `characters.hurl` 7/7, `blocks.hurl` 4/4, `health`/`preferences`/`session` green. `token_status` confirmed live on `/api/v1/me`. (`session.hurl` needs `--no-cookie-store` per hurl's shared cookie jar; `account.hurl` skipped — it deletes the account.)
- [x] 9.6 `pnpm run test -- --run` (from `frontend/`) — Vitest unit/component tests pass (177). *(`--filter frontend` does not work: there is no root pnpm workspace manifest; run from `frontend/`.)*
- [x] 9.7 `pnpm run check` (from `frontend/`) — svelte-check (types + paraglide compile) passes, 0 errors/0 warnings.
- [x] 9.8 `pnpm run test:e2e` (from `frontend/`) — Playwright e2e tests pass (17).

> This change touches frontend code (token-state rendering + admin Characters tab), so per `CLAUDE.md` the full frontend verification trio (9.6–9.8) is mandatory in addition to the backend checks.
