## 1. Paraglide setup (per `sveltekit-node` skill)

- [ ] 1.1 Add `@inlang/paraglide-js` and an inlang project (`project.inlang/settings.json`) with `baseLocale: en` and `locales: [en]`.
- [ ] 1.2 Wire the Paraglide Vite plugin so messages compile to `$lib/paraglide/messages` (and the runtime to `$lib/paraglide/runtime`); add the generated output to the appropriate ignore rules.
- [ ] 1.3 Configure the locale resolution strategy `['cookie', 'preferredLanguage', 'baseLocale']`.
- [ ] 1.4 Add the SvelteKit server hook (`hooks.server.ts`) using Paraglide's `paraglideMiddleware`/`reroute` integration so the active locale is resolved per request and `<html lang>` is server-rendered correctly. (This file does not exist yet — it is introduced here.)

## 2. Message catalogue

- [ ] 2.1 Establish the message catalogue (`messages/en.json`) and a key naming convention (e.g. dotted, feature-scoped keys).
- [ ] 2.2 Document the convention briefly so new strings are added consistently.

## 3. Locale persistence on the account-preferences substrate

> The substrate ships in the archived `accessibility-preferences` change.

- [ ] 3.1 Add `locale` to the recognised + validated keys of the backend preferences service (validate against the supported-locale set); no new column or endpoint — extend `services/preferences.rs` and its tests, regenerate the sqlx cache if needed.
- [ ] 3.2 Add `locale` to the frontend preference schema/types and the preferences store, persisted via `preferences.locale` (localStorage + backend sync, login reconciliation) like the other keys.
- [ ] 3.3 Bridge the store to Paraglide: whenever `locale` changes (Apply, login reconcile), write Paraglide's locale cookie so SSR language always matches the stored preference.

## 4. String replacement (the bulk of the work)

- [ ] 4.1 Replace **all** hardcoded user-facing strings across the frontend with Paraglide message calls (`m.*`).
- [ ] 4.2 Populate `messages/en.json` with every extracted string.
- [ ] 4.3 Update component tests that assert on literal copy to tolerate message-driven text.

## 5. Locale selection UI

- [ ] 5.1 Add a locale selector to the `/preferences` page, staged like the other preferences (it is not layout-altering, so it does not need the contrast/size-proof recovery treatment, but it commits via the same Apply flow).
- [ ] 5.2 On commit, persist as `preferences.locale` and set the Paraglide cookie so the new locale takes effect on the next render.

## 6. Verification

- [ ] 6.1 `svelte-check`, `vitest run`, `pnpm build` green; backend `cargo test` + fmt + sqlx cache green if the service was touched.
