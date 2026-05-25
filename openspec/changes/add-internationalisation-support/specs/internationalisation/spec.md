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
The system SHALL persist the user's locale preference to their account so it is restored across sessions and devices.

#### Scenario: Locale preference saved
- **WHEN** a user changes their locale preference
- **THEN** the new locale SHALL be saved to the user's account via the API

#### Scenario: Locale preference restored on login
- **WHEN** an authenticated user loads the application
- **THEN** the locale SHALL be set to the user's stored preference
