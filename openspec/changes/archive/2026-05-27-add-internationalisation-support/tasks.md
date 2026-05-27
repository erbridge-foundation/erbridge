## 1. Paraglide setup (per `sveltekit-node` skill)

- [x] 1.1 Add `@inlang/paraglide-js` and an inlang project (`project.inlang/settings.json`) with `baseLocale: en` and `locales: [en]`.
- [x] 1.2 Wire the Paraglide Vite plugin so messages compile to `$lib/paraglide/messages` (and the runtime to `$lib/paraglide/runtime`); add the generated output to the appropriate ignore rules.
- [x] 1.3 Configure the locale resolution strategy `['cookie', 'preferredLanguage', 'baseLocale']`.
- [x] 1.4 Add the SvelteKit server hook (`hooks.server.ts`) using Paraglide's `paraglideMiddleware` integration so the active locale is resolved per request and `<html lang>` is server-rendered correctly (via the `%paraglide.lang%` placeholder in app.html). No `reroute`/`src/hooks.ts` — the `url` strategy is not used.

## 2. Message catalogue

- [x] 2.1 Establish the message catalogue (`messages/en.json`) and a key naming convention (flat snake_case, feature-scoped keys).
- [x] 2.2 Document the convention briefly so new strings are added consistently (`messages/README.md`).

## 3. Locale persistence on the account-preferences substrate

> The substrate ships in the archived `accessibility-preferences` change.

- [x] 3.1 Add `locale` as a typed key on the backend preferences DTO layer, where preference validation actually lives (the service does no value-validation; `PreferencesPatch` is `#[serde(deny_unknown_fields)]`, so an unknown key is rejected at deserialisation). Per the `rust-rest-api` skill's DTO/service split:
  - In `dto/preferences.rs`: add a `Locale` enum (the supported-locale set, `#[serde(rename_all = "snake_case")]`) and add `locale` fields to `PreferencesDto` (with a default), `PreferencesDto::default()`, `PreferencesPatch`, and `PreferencesPatch::is_empty()`.
  - In `services/preferences.rs`: read `locale` in `dto_from_bag` and emit it in `patch_to_json`, mirroring the other keys.
  - **Invert the two tests that currently encode "locale is foreign":** `dto/preferences.rs::patch_rejects_unknown_key` (locale must now deserialise) and `services/preferences.rs::dto_from_bag_ignores_unknown_and_invalid_values` (locale must now be read). Keep an unknown-key rejection test using a genuinely unknown key. The `does_not_clobber_foreign_keys` test can keep using a different foreign key.
  - No new column or endpoint; regenerate the sqlx cache if needed (no query change is expected, since the bag is JSONB).
- [x] 3.2 Add `locale` to the frontend preference schema/types and the preferences store, persisted via `preferences.locale` (localStorage + backend sync, login reconciliation) like the other keys.
- [x] 3.3 Bridge the store to Paraglide: whenever `locale` changes (Apply, login reconcile), write Paraglide's locale cookie so SSR language always matches the stored preference.

## 4. String replacement (the bulk of the work)

- [x] 4.1 Replace **all** hardcoded user-facing strings across the frontend with Paraglide message calls (`m.*`). (Kept: the `E-R BRIDGE` wordmark, the CCP-provided SSO image `alt`, and the maintainer-curated acknowledgement entries on /about — curated data, not UI chrome.)
- [x] 4.2 Populate `messages/en.json` with every extracted string.
- [x] 4.3 Update component tests that assert on literal copy to tolerate message-driven text. (Messages resolve to identical English, so existing `getByText`/`getByRole` assertions still match; only the `Preferences` type fixtures needed the new required `locale` key.)

## 5. Preferences page tabs + locale selection UI

> Tabs are a presentation layer over the existing single staged batch: one `staged` set, one `dirty` flag, one Apply/Discard/Reset bar across both tabs. The existing staging / `beforeNavigate` / teardown machinery is unchanged — switching tabs neither commits nor discards.

- [x] 5.1 Split `/preferences` into a tabbed interface: a "General" tab and an "Accessibility" tab. Move the existing accessibility controls (text size, reduce motion, high contrast, larger targets, dyslexia font) under "Accessibility". Keep the shared Apply/Discard/Reset action bar below the tabs (it stays the contrast/size-proof recovery surface). Update the page `<h1>`/intro copy that is currently accessibility-specific to be page-level.
- [x] 5.2 Add the locale selector to the "General" tab, staged like the other preferences via the shared `staged`/`select` flow.
- [x] 5.3 On Apply (the shared commit), persist the locale as `preferences.locale` and set the Paraglide cookie so the new locale takes effect on the next render.
- [x] 5.4 Update `page.svelte.test.ts`: tab switching keeps staged changes and the shared action bar; locale selects/stages/applies on the General tab; accessibility controls render on the Accessibility tab.

## 6. Verification

- [x] 6.1 `svelte-check`, `vitest run`, `pnpm build` green; backend `cargo test` + fmt + sqlx cache green if the service was touched.
