## ADDED Requirements

### Requirement: Frontend i18n infrastructure
The frontend SHALL use an i18n library to manage all user-facing strings via a message catalogue, replacing hardcoded English literals.

#### Scenario: Translated string rendered
- **WHEN** a component renders a user-facing string
- **THEN** the string SHALL be resolved via the i18n translation function using a message key

### Requirement: Locale detection
The system SHALL detect the user's preferred locale from browser settings on first visit and apply it as the active locale.

#### Scenario: Browser locale applied on first visit
- **WHEN** a user visits the application for the first time with no stored locale preference
- **THEN** the active locale SHALL be set to the user's browser locale, falling back to `en` if unsupported

### Requirement: Locale preference persistence
The system SHALL persist the user's locale preference as the `locale` key on the account-preferences substrate (the `preferences` JSONB bag, written via `PATCH /api/v1/me/preferences`), NOT as a dedicated locale column or endpoint. It SHALL therefore inherit the substrate's behaviour: localStorage-first for anonymous visitors, backend sync for authenticated users, and login reconciliation (server wins, else push-local-on-first-login). The locale value SHALL be applied to `<html lang>` before first paint via the shared `app.html` bootstrap.

#### Scenario: Locale preference saved
- **WHEN** a user changes their locale preference
- **THEN** the new locale SHALL be written as `preferences.locale` (to localStorage, and PATCHed to `/api/v1/me/preferences` for authenticated users)

#### Scenario: Locale preference restored on login
- **WHEN** an authenticated user loads the application
- **THEN** the locale SHALL be set from `preferences.locale` resolved through the account-preferences substrate

#### Scenario: Locale applied before paint
- **WHEN** a returning visitor with a stored `preferences.locale` loads any page
- **THEN** `<html lang>` SHALL reflect that locale on first paint, with no flash of the default language
