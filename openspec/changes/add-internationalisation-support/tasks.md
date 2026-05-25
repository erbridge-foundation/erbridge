## 1. Frontend i18n infrastructure

- [ ] 1.1 Add i18n library dependency to the frontend
- [ ] 1.2 Create message catalogue structure and key naming convention
- [ ] 1.3 Wire i18n initialisation into the SvelteKit layout

## 2. Locale detection and preference

> Depends on `accessibility-preferences` (preference substrate). Apply that change first.

- [ ] 2.1 Implement browser locale detection with `en` fallback (a runtime default; not a stored value until the user chooses)
- [ ] 2.2 Add `locale` to the recognised + validated keys of the backend preferences service (validate against the supported-locale set); no new column or endpoint — extend `services/preferences.rs` and its tests
- [ ] 2.3 Read/write locale via the existing frontend preferences store (`preferences.locale`); restore it through the substrate's login reconciliation on authenticated load
- [ ] 2.4 Extend the `app.html` preference bootstrap to apply `preferences.locale` to `<html lang>` before paint

## 3. String replacement

- [ ] 3.1 Replace hardcoded user-facing strings in frontend components with translation keys
- [ ] 3.2 Populate `en` message catalogue with all extracted strings

## 4. Locale selection UI

- [ ] 4.1 Add a locale selector to the `/preferences` page (the account-preferences UI surface)
- [ ] 4.2 Wire the selector to the preferences store so it persists as `preferences.locale` (localStorage + backend sync); locale is not layout-altering, so it does not need the auto-revert countdown
