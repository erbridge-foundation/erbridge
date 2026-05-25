## ADDED Requirements

### Requirement: `locale` is a recognised account preference

The account-preferences substrate SHALL recognise `locale` as a valid preference key. The preferences service SHALL validate a supplied `locale` against the set of supported locales, rejecting an unsupported value with 400 (consistent with how other preference keys are validated). `locale` is stored in the existing `preferences` JSONB bag and read/written via the existing `GET`/`PATCH /api/v1/me/preferences` endpoints — no new column or endpoint is introduced.

#### Scenario: Supported locale accepted
- **WHEN** an authenticated account PATCHes `{ "locale": "<supported-locale>" }` to `/api/v1/me/preferences`
- **THEN** the response SHALL be 200 and `preferences.locale` SHALL be updated, leaving other keys unchanged

#### Scenario: Unsupported locale rejected
- **WHEN** a PATCH sets `locale` to a value outside the supported locale set
- **THEN** the response SHALL be 400 and the stored preferences SHALL be unchanged
