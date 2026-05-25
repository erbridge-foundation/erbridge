## 1. Frontend i18n infrastructure

- [ ] 1.1 Add i18n library dependency to the frontend
- [ ] 1.2 Create message catalogue structure and key naming convention
- [ ] 1.3 Wire i18n initialisation into the SvelteKit layout

## 2. Locale detection and preference

- [ ] 2.1 Implement browser locale detection with `en` fallback
- [ ] 2.2 Add locale preference read/write to the account API (backend)
- [ ] 2.3 Restore stored locale preference on authenticated page load

## 3. String replacement

- [ ] 3.1 Replace hardcoded user-facing strings in frontend components with translation keys
- [ ] 3.2 Populate `en` message catalogue with all extracted strings

## 4. Locale selection UI

- [ ] 4.1 Add locale selector component to user settings
- [ ] 4.2 Wire locale selector to persist preference via account API
