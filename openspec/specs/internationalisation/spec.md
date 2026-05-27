## Purpose

Defines the frontend internationalisation (i18n) infrastructure: Paraglide as the message-catalogue library, per-request server-side locale resolution (cookie → `Accept-Language` → base locale `en`, no URL prefix), persistence of the locale preference on the account-preferences substrate, and the locale-selection UI on the preferences page. English is the only production locale; the infrastructure supports adding more.

## Requirements

### Requirement: Frontend i18n infrastructure
The frontend SHALL use Paraglide (`@inlang/paraglide-js`) to manage all user-facing strings via a message catalogue, replacing hardcoded English literals. The active locale SHALL be resolved per request on the server so that server-rendered output is in the correct language.

#### Scenario: Translated string rendered
- **WHEN** a component renders a user-facing string
- **THEN** the string SHALL be resolved via a Paraglide message function using a message key, not a hardcoded literal

### Requirement: Locale resolution
The system SHALL resolve the active locale using the ordered strategy cookie → browser `Accept-Language` → base locale (`en`). The locale SHALL NOT be encoded in the URL path. On a request bearing the locale cookie, that locale SHALL be used; otherwise the browser's preferred language SHALL be used if supported, falling back to `en`.

#### Scenario: Cookie locale used when present
- **WHEN** a request carries the locale cookie
- **THEN** the active locale SHALL be the cookie's value and the server-rendered page SHALL be in that language

#### Scenario: Browser locale used on first visit
- **WHEN** a user visits for the first time with no locale cookie and no stored preference
- **THEN** the active locale SHALL be derived from `Accept-Language`, falling back to `en` if unsupported

### Requirement: Locale preference persistence
The system SHALL persist the user's locale preference as the `locale` key on the account-preferences substrate (the `preferences` JSONB bag, written via `PATCH /api/v1/me/preferences`), NOT as a dedicated locale column or endpoint. It SHALL therefore inherit the substrate's behaviour: localStorage-first for anonymous visitors, backend sync for authenticated users, and login reconciliation (server wins, else push-local-on-first-login). Whenever the locale preference changes, the system SHALL write Paraglide's locale cookie so the per-request server resolution matches the stored preference.

#### Scenario: Locale preference saved
- **WHEN** a user changes their locale preference
- **THEN** the new locale SHALL be written as `preferences.locale` (to localStorage, and PATCHed to `/api/v1/me/preferences` for authenticated users)

#### Scenario: Locale preference restored on login
- **WHEN** an authenticated user loads the application
- **THEN** the locale SHALL be set from `preferences.locale` resolved through the account-preferences substrate

#### Scenario: Stored locale rendered server-side without a flash
- **WHEN** a returning visitor whose locale cookie reflects their stored `preferences.locale` loads any page
- **THEN** the server SHALL render the page in that locale on the first pass, with no flash of the default language on hydration

### Requirement: Locale selection on the preferences page
The `/preferences` page SHALL organise its controls into tabs. A "General" tab SHALL contain the locale selector; an "Accessibility" tab SHALL contain the existing accessibility controls. The tabs SHALL share a single staged set and a single Apply/Discard/Reset action bar — switching tabs SHALL NOT commit, discard, or reset staged changes, and `dirty` SHALL reflect staged changes on either tab. Selecting a locale SHALL stage it (previewed live) and, on Apply, persist it as `preferences.locale` and write Paraglide's locale cookie.

#### Scenario: Locale staged and applied from the General tab
- **WHEN** a user selects a locale on the General tab and clicks Apply
- **THEN** the locale SHALL be persisted as `preferences.locale` and Paraglide's locale cookie SHALL be written so the new locale takes effect on the next render

#### Scenario: Staged changes survive tab switches
- **WHEN** a user stages a change on one tab and switches to the other tab
- **THEN** the staged change SHALL remain staged, the shared Apply/Discard bar SHALL stay visible, and `dirty` SHALL remain true until Apply, Discard, or Reset
