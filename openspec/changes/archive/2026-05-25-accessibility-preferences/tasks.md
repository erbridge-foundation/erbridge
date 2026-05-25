## 1. Backend: preference substrate (per `rust-rest-api` skill layout)

- [x] 1.1 Add migration `…_add_account_preferences.sql`: `ALTER TABLE account ADD COLUMN preferences JSONB NOT NULL DEFAULT '{}'::jsonb`
- [x] 1.2 `db/preferences.rs`: `get_preferences(account_id)` and `merge_preferences(account_id, patch)` (JSONB merge), with `#[sqlx::test]` coverage
- [x] 1.3 `dto/preferences.rs`: `PreferencesDto` (+ `ToSchema`) and a `PreferencesPatch` input type
- [x] 1.4 `services/preferences.rs`: validate keys + enum values, reject unknown/invalid with the appropriate `AppError`; unit tests for validation + `#[sqlx::test]` for merge semantics
- [x] 1.5 `handlers/api/v1/preferences.rs` (or extend `me.rs`): `GET` and `PATCH /api/v1/me/preferences` behind `AuthenticatedAccount`, with `#[utoipa::path]` annotations
- [x] 1.6 Register routes; confirm they are covered by the fail-closed auth-coverage test
- [x] 1.7 HURL integration tests under `tests/hurl/`: authed GET, authed PATCH partial-merge, unknown-key 400, invalid-value 400, unauthenticated 401

## 2. Frontend: preference store + sync (per `sveltekit-node` skill)

- [x] 2.1 `lib/preferences/`: define the preference schema (keys, allowed values, defaults) and the `PREFERENCE_REVERT_SECONDS` constant (default 10)
- [x] 2.2 Generic preferences store: localStorage-first read/write, `$state`/`$derived` runes, applies values to `document.documentElement` (`font-size` + `data-*`)
- [x] 2.3 Backend sync: `GET`/`PATCH` via `lib/api.ts`; login reconciliation (server wins, else push-local-on-empty-server)
- [x] 2.4 Unit tests for store: default resolution, localStorage round-trip, reconciliation branches

## 3. No-FOUC bootstrap

- [x] 3.1 Add the inline `<script>` in `app.html` that reads localStorage and applies non-`auto` values to `<html>` before paint
- [x] 3.2 Hydrate the store from the same source on app start without re-flashing

## 4. CSS: apply preferences + OS-default media queries

- [x] 4.1 `text_size` steps via `html { font-size }`; `auto`/`regular` = 100%, define `small`/`large` percentages
- [x] 4.2 `data-reduce-motion` + `@media (prefers-reduced-motion: reduce)` (auto default)
- [x] 4.3 `data-high-contrast` + `@media (prefers-contrast: more)` token overrides (auto default)
- [x] 4.4 `data-large-targets`: minimum interactive target sizing
- [x] 4.5 `data-dyslexia-font`: bundle/declare the alternative typeface and switch `font-family`

## 5. /preferences page + user-menu

- [x] 5.1 `routes/preferences/+page.svelte`: controls for all five preferences; reachable anonymously
- [x] 5.2 `UserMenu.svelte`: turn the `preferences` placeholder into `<a href="/preferences">`; leave `settings` disabled; remove the TODO breadcrumb
- [x] 5.3 Component tests: controls render, reflect current values, enabled menu link

## 6. Auto-reverting confirmation (safe mode)

- [x] 6.1 Confirmation component: live-preview + `PREFERENCE_REVERT_SECONDS` countdown + Keep / Revert-now; styled to resist the previewed change (fixed `px` sizing, guaranteed contrast)
- [x] 6.2 Wire it to the layout-altering prefs only (`text_size`, `high_contrast`, `large_targets`, `dyslexia_font`); `reduce_motion` commits immediately
- [x] 6.3 Commit only on Keep (write localStorage + sync); auto-revert on timeout; nothing persisted during countdown
- [x] 6.4 Tests: revert-on-timeout, keep-commits, reduce_motion bypasses countdown, nothing-persisted-mid-countdown

## 7. Reduce-motion audit

- [x] 7.1 Audit every animation/transition (pulsing `connected` dot, character-grid hover, others) and gate them on reduce-motion
- [x] 7.2 Document the motion-gating mechanism so new code respects the preference by default

## 8. Spec + cross-reference housekeeping

- [x] 8.1 Verify the `project-infrastructure` Typography/Motion amendments match the shipped CSS
- [x] 8.2 Leave a cross-reference note for `add-internationalisation-support` that locale should become `preferences.locale` on this substrate (no new backend column/endpoint)
