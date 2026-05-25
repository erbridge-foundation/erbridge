## Why

The application currently has no internationalisation (i18n) infrastructure, meaning all user-facing strings are hardcoded in English. Adding i18n support enables the app to be localised into multiple languages in the future.

## What Changes

- Introduce an i18n library and locale message catalogue to the frontend
- Replace hardcoded user-facing strings with translation keys
- Add a locale selection mechanism (defaulting to browser locale)
- Expose locale preference storage (user setting, persisted server-side)

## Capabilities

### New Capabilities

- `internationalisation`: Frontend i18n infrastructure — library setup, message catalogue, locale detection, and locale preference persistence

### Modified Capabilities

## Impact

- Frontend: new i18n dependency, all user-facing string literals replaced with translation calls
- Backend: new user locale preference field on account; new API endpoint or extension to account-management for reading/writing locale preference
- Existing specs: `account-management` may require a delta if locale is stored server-side
