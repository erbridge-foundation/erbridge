# Tasks — surface-add-character-conflict

## 1. Backend conflict outcome

- [x] 1.1 Add `SsoOutcome::BoundElsewhere` and the in-transaction bound-elsewhere check at the top of `complete_sso_callback`'s add-character branch (before `upsert_tokens`); roll back the main tx and record the audit row in its own short tx, mirroring the blocked flow
- [x] 1.2 Add `AuditEvent::CharacterAddRejectedBoundElsewhere { account_id, eve_character_id }` with event type, details (no owning-account leak), and character target; unit tests for all three methods
- [x] 1.3 Map the outcome in `handlers/auth.rs::callback` to a 303 redirect to `{return_to | /characters}?add_conflict=bound_elsewhere`, preserving the session cookie refresh
- [x] 1.4 Integration tests: bound-elsewhere add leaves account A's row byte-identical, adds nothing to B, emits exactly the rejection event; ordinary add-character and orphan-claim flows unchanged

## 2. Frontend notice

- [x] 2.1 Characters page: read `add_conflict=bound_elsewhere`, render a dismissible notice, strip the query flag via `replaceState` after first render
- [x] 2.2 i18n keys for the notice in `en`/`de`/`fr`
- [x] 2.3 Vitest coverage for the notice rendering/flag-stripping; extend the e2e mock backend with the conflict redirect and add a Playwright spec

## 3. Verification

- [x] 3.1 `cargo fmt` && `cargo clippy --all-targets -- -D warnings` && `cargo test` from `backend/`; `cargo sqlx prepare -- --all-targets` if queries changed
- [x] 3.2 `pnpm --filter frontend test` — Vitest unit/component tests
- [x] 3.3 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile)
- [x] 3.4 `pnpm --filter frontend run test:e2e` — Playwright e2e tests
- [x] 3.5 Live smoke test on the dev compose stack: attempt to add a character already on a second test account; confirm notice, unchanged bindings, and the audit row
