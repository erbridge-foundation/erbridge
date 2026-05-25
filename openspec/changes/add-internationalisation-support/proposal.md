## Why

The application currently has no internationalisation (i18n) infrastructure, meaning all user-facing strings are hardcoded in English. Adding i18n support enables the app to be localised into multiple languages in the future.

## What Changes

- Introduce an i18n library and locale message catalogue to the frontend
- Replace hardcoded user-facing strings with translation keys
- Add a locale selection mechanism (defaulting to browser locale)
- Persist the locale preference as **`preferences.locale`** on the existing account-preferences substrate (the `preferences` JSONB column + `GET`/`PATCH /api/v1/me/preferences`), introduced by the `accessibility-preferences` change. **No new account column and no new endpoint** — locale is one more key in the preference bag, with the same localStorage-first-with-backend-sync behaviour and apply-before-paint handling (`<html lang>`).

**Depends on `accessibility-preferences`** for the preference substrate (column, endpoint, frontend store). That change should be applied first; if it is not, this change must add the substrate itself rather than a locale-specific column/endpoint.

## Capabilities

### New Capabilities

- `internationalisation`: Frontend i18n infrastructure — library setup, message catalogue, locale detection, and locale preference persistence (the latter delegated to the `account-preferences` substrate via `preferences.locale`)

### Modified Capabilities

- `account-preferences`: gains `locale` as a recognised preference key (validated against the supported locale set). No structural change — the substrate already stores arbitrary keys in the `preferences` JSONB bag.

## Impact

- Frontend: new i18n dependency, all user-facing string literals replaced with translation calls; locale read/written through the existing preferences store (not a new locale-specific API client)
- Backend: **no new column, no new endpoint.** `locale` is added to the recognised/validated keys of the preferences service introduced by `accessibility-preferences`
- Existing specs: no `account-management` delta needed — locale rides on `account-preferences`
