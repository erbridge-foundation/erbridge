## Purpose

Defines how account preferences are stored and accessed on the backend: a JSONB preference bag on the `account` table, the authenticated `GET`/`PATCH /api/v1/me/preferences` endpoints (validated partial merge), and the anonymous-with-sync model where the browser's `localStorage` is the edge source of truth while authenticated users sync to the backend so preferences persist across devices. Also defines login reconciliation between local and server preferences.

## Requirements

### Requirement: Account preferences are stored as a JSONB bag

The `account` table SHALL have a `preferences` column of type JSONB, `NOT NULL DEFAULT '{}'::jsonb`. Preferences are a key/value bag keyed by preference name. Adding a new preference key SHALL NOT require a schema migration. Existing accounts SHALL default to `{}`, which means "all preferences at their default", with no behavioural change until a preference is set.

#### Scenario: New account has empty preferences
- **WHEN** a new account is created
- **THEN** its `preferences` column SHALL be `{}`

#### Scenario: Adding a preference key needs no migration
- **WHEN** a new preference key is introduced in a later change
- **THEN** it SHALL be stored under the existing `preferences` JSONB column with no `ALTER TABLE`

### Requirement: Preferences are readable via an authenticated endpoint

The backend SHALL expose `GET /api/v1/me/preferences`, behind the `AuthenticatedAccount` extractor, returning the authenticated account's preference bag in the standard response envelope. An unauthenticated request SHALL return 401.

#### Scenario: Authenticated read
- **WHEN** an authenticated account requests `GET /api/v1/me/preferences`
- **THEN** the response SHALL be 200 with the account's stored preferences in `data`

#### Scenario: Unauthenticated read rejected
- **WHEN** a request to `GET /api/v1/me/preferences` has no valid session or bearer token
- **THEN** the response SHALL be 401 with the standard error envelope

### Requirement: Preferences are updated via a validated partial merge

The backend SHALL expose `PATCH /api/v1/me/preferences`, behind the `AuthenticatedAccount` extractor, accepting a partial set of preference keys. The supplied keys SHALL be merged into the existing preference bag (keys not present in the request are left unchanged). The endpoint SHALL validate every supplied key and value at the service layer: an unknown key, or a known key with a value outside its allowed set, SHALL be rejected with 400 and the existing preferences left unchanged. On success it SHALL return 200 with the full merged preference set.

#### Scenario: Partial merge preserves other keys
- **WHEN** an authenticated account PATCHes `{ "text_size": "large" }` while `reduce_motion` is already set
- **THEN** `text_size` SHALL be updated and `reduce_motion` SHALL be unchanged, and the response SHALL contain the full merged set

#### Scenario: Unknown key rejected
- **WHEN** a PATCH body contains a key that is not a recognised preference
- **THEN** the response SHALL be 400 and the stored preferences SHALL be unchanged

#### Scenario: Invalid value rejected
- **WHEN** a PATCH body sets a known key to a value outside its allowed set
- **THEN** the response SHALL be 400 and the stored preferences SHALL be unchanged

### Requirement: Preferences work anonymously via the browser with backend sync

Preferences SHALL be usable without an authenticated account. The browser's `localStorage` SHALL be the source of truth at the edge: it is readable synchronously and applied before first paint. For authenticated users, preference changes SHALL additionally be synced to the backend so they persist across devices.

#### Scenario: Anonymous user sets a preference
- **WHEN** a visitor with no account changes a preference
- **THEN** the preference SHALL be stored in `localStorage` and applied, with no backend call

#### Scenario: Authenticated change syncs to backend
- **WHEN** an authenticated user commits a preference change
- **THEN** the value SHALL be written to `localStorage` and PATCHed to the backend

### Requirement: Login reconciliation prefers the server, but pushes local on first login

On authenticated load the frontend SHALL reconcile `localStorage` with the server's stored preferences. If the server's preferences are empty AND `localStorage` holds values, the `localStorage` values SHALL be pushed up to the server (preserving an anonymous user's setup when they first sign in). Otherwise the server's values SHALL win and overwrite `localStorage`.

#### Scenario: First login pushes anonymous setup up
- **WHEN** a user who configured preferences while anonymous logs into an account whose server preferences are empty
- **THEN** the `localStorage` preferences SHALL be pushed to the server and retained

#### Scenario: Existing server preferences win
- **WHEN** an authenticated user loads the app and the server already has stored preferences
- **THEN** the server's preferences SHALL overwrite `localStorage`

### Requirement: `locale` is a recognised account preference

The account-preferences substrate SHALL recognise `locale` as a valid preference key. The preferences service SHALL validate a supplied `locale` against the set of supported locales, rejecting an unsupported value with 400 (consistent with how other preference keys are validated). `locale` is stored in the existing `preferences` JSONB bag and read/written via the existing `GET`/`PATCH /api/v1/me/preferences` endpoints — no new column or endpoint is introduced.

#### Scenario: Supported locale accepted
- **WHEN** an authenticated account PATCHes `{ "locale": "<supported-locale>" }` to `/api/v1/me/preferences`
- **THEN** the response SHALL be 200 and `preferences.locale` SHALL be updated, leaving other keys unchanged

#### Scenario: Unsupported locale rejected
- **WHEN** a PATCH sets `locale` to a value outside the supported locale set
- **THEN** the response SHALL be 400 and the stored preferences SHALL be unchanged
