## Why

The application currently has no internationalisation (i18n) infrastructure, meaning all user-facing strings are hardcoded in English. Adding i18n support enables the app to be localised into multiple languages in the future.

## What Changes

- Adopt **Paraglide** (`@inlang/paraglide-js`) as the i18n library, with its SvelteKit integration and a message catalogue. Paraglide is compile-time (messages become tree-shakeable functions) and resolves the active locale per request server-side, so SSR renders the right language with no hydration flash.
- Resolve the active locale with strategy **`['cookie', 'preferredLanguage', 'baseLocale']`** — a server-readable cookie (the user's choice), then the browser `Accept-Language` header, then `en`. **No locale in the URL** (no `/en/` path prefixes): E-R Bridge is an authenticated tool, so the SEO/shareable-link benefits don't apply and the routing cost isn't justified for an English-only launch. `'url'` can be added later if the app goes public.
- Replace **all** hardcoded user-facing strings with Paraglide message keys (the bulk of the work).
- Persist the locale preference as **`preferences.locale`** on the existing `account-preferences` substrate (the `preferences` JSONB column + `GET`/`PATCH /api/v1/me/preferences`). **No new account column and no new endpoint** — locale is one more validated key in the preference bag, with the same localStorage-first-with-backend-sync behaviour.
- **Bridge** the preferences store to Paraglide's locale cookie: whenever `locale` changes (commit, login reconcile), the store writes the cookie so the server-rendered language always matches the stored preference. This is the single integration point.

**Depends on `accessibility-preferences`** for the preference substrate (column, endpoint, frontend store), now archived. If that substrate were absent, this change would have to add it rather than a locale-specific column/endpoint.

## Capabilities

### New Capabilities

- `internationalisation`: Frontend i18n infrastructure — library setup, message catalogue, locale detection, and locale preference persistence (the latter delegated to the `account-preferences` substrate via `preferences.locale`)

### Modified Capabilities

- `account-preferences`: gains `locale` as a recognised preference key (validated against the supported locale set). No structural change — the substrate already stores arbitrary keys in the `preferences` JSONB bag.

## Impact

- Frontend: new i18n dependency, all user-facing string literals replaced with translation calls; locale read/written through the existing preferences store (not a new locale-specific API client)
- Backend: **no new column, no new endpoint.** `locale` is added to the recognised/validated keys of the preferences service introduced by `accessibility-preferences`
- Existing specs: no `account-management` delta needed — locale rides on `account-preferences`
