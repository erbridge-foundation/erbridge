# Tasks — harden-auth-flow

## 1. State cookie (login CSRF binding)

- [ ] 1.1 Add `auth_state` cookie helpers to `backend/src/handlers/cookie.rs` (`set_auth_state_cookie`, `clear_auth_state_cookie`, `extract_auth_state`) with `HttpOnly; SameSite=Lax; Secure; Path=/auth; Max-Age=900`, plus unit tests for attributes and extraction
- [ ] 1.2 Set the cookie in `login` and `add_character` (`backend/src/handlers/auth.rs`); value is the generated `csrf_state`
- [ ] 1.3 In `callback`, require cookie == `state` query param before `InflightStore::take`; respond 400 on absent/mismatch; clear the cookie on all outcomes (success, blocked redirect, errors); remove the now-dead `inflight.csrf_state != query.state` re-check
- [ ] 1.4 Integration tests: callback without cookie → 400; mismatching cookie → 400; happy path sets session cookie and clears `auth_state`

## 2. In-flight store TTL + cap

- [ ] 2.1 Add `created_at: std::time::Instant` to `InflightRecord`; `InflightStore::take` returns `None` for records older than 15 min; `add` sweeps expired entries and enforces the 10 000-record cap (refuse insert when full), returning a result the login handler maps to an error response
- [ ] 2.2 Unit tests: expired record not returned; sweep evicts expired on insert; cap refusal path

## 3. Secure session cookie

- [ ] 3.1 Add `Secure` to `set_session_cookie` and `clear_session_cookie` in `backend/src/handlers/cookie.rs`; update unit tests
- [ ] 3.2 Confirm dev compose stack serves over a secure context (Traefik TLS or localhost); note in `backend/README.md` if any dev-setup step changes

## 4. Logout method change

- [ ] 4.1 Change `/auth/logout` route registration to `post(...)` in `backend/src/lib.rs`; verify GET yields 405
- [ ] 4.2 Replace the frontend logout link with a minimal POST form (GlobalNav and any other `/auth/logout` references); style to match the existing link
- [ ] 4.3 Update Playwright e2e specs and the e2e mock backend for POST logout

## 5. Spec/test sync

- [ ] 5.1 Update HURL tests (`backend/tests/hurl/session.hurl` and any auth coverage) for state-cookie behaviour, Secure attribute, and POST logout
- [ ] 5.2 Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` from `backend/`; regenerate sqlx cache only if queries changed (none expected)

## 6. Verification

- [ ] 6.1 `pnpm --filter frontend test` — Vitest unit/component tests
- [ ] 6.2 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile)
- [ ] 6.3 `pnpm --filter frontend run test:e2e` — Playwright e2e tests
- [ ] 6.4 Live smoke test against the dev compose stack: full SSO login round-trip, add-character, logout via UI, and a callback replay in a second browser profile is rejected with 400
