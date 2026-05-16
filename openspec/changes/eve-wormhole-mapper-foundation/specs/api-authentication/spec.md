## ADDED Requirements

All `/api/*` request and response bodies in this capability conform to the `api-contract` spec (success envelope, error envelope, canonical error codes, RFC 3339 timestamps). Endpoint shapes below describe the contents of `data` for success and the contents of `error.details` (where applicable) for failure.

### Requirement: API key format
API keys SHALL have the format `erb_<body>` where `<body>` is exactly 43 characters of unpadded base64url (`[A-Za-z0-9_-]`). The full key SHALL be 47 characters. The `<body>` SHALL be derived from 32 cryptographically-random bytes (256 bits) drawn from a CSPRNG. Keys SHALL be hashed for storage with SHA-256 over the entire key (prefix included), encoded as lowercase hex.

#### Scenario: Generated key matches the format
- **WHEN** the backend generates a new API key
- **THEN** the returned plaintext starts with `erb_`, is exactly 47 characters long, and the body characters are valid unpadded base64url

#### Scenario: key_hash is SHA-256 of the full key
- **WHEN** a key is generated and stored
- **THEN** `key_hash` equals `lowercase_hex(SHA-256(plaintext_key))` and matches no other row

### Requirement: API key authentication on /api/*
The backend SHALL authenticate `/api/*` requests using one of two methods, tried in order:

1. **Bearer token** — if `Authorization: Bearer <value>` is present and `<value>` begins with `erb_`, the backend SHALL hash it with SHA-256 and look up `api_key.key_hash`. On a match with `expires_at IS NULL OR expires_at > now()`, the request is authenticated:
   - `scope = 'account'`: authenticated as the row's `account_id`; the request has the same permissions as that account's session would have.
   - `scope = 'server'`: authenticated as a server-scoped caller (no `account_id`); routes in this change SHALL reject server-scoped callers with HTTP 403.
2. **Session cookie** — if no Bearer header was present (or it did not match the prefix), the backend SHALL fall back to the session cookie set by the SSO flow.

If neither method succeeds, the backend SHALL respond with HTTP 401.

#### Scenario: Valid account-scoped key authenticates request
- **WHEN** a `GET /api/v1/keys` request arrives with `Authorization: Bearer <valid account-scoped key>`
- **THEN** the request is authenticated as the key's `account_id` and proceeds

#### Scenario: Valid session cookie still works when no bearer header is present
- **WHEN** a `GET /api/v1/keys` request arrives with no `Authorization` header but a valid session cookie
- **THEN** the request is authenticated as the session's account and proceeds

#### Scenario: Expired key is rejected
- **WHEN** a request presents a key whose `expires_at` is in the past
- **THEN** the backend responds with HTTP 401 and does not authenticate the request

#### Scenario: Unknown key is rejected
- **WHEN** a request presents a key whose SHA-256 hash is not in `api_key`
- **THEN** the backend responds with HTTP 401

#### Scenario: Server-scoped key on a normal route is rejected
- **WHEN** any `/api/*` route in this change receives a request authenticated by a `scope = 'server'` key
- **THEN** the backend responds with HTTP 403

#### Scenario: API key in query string is not accepted
- **WHEN** a request supplies an API key as `?api_key=<value>` with no `Authorization` header
- **THEN** the backend does not authenticate the request (responds 401 absent a session cookie)

#### Scenario: Bearer header not starting with erb_ is ignored
- **WHEN** an `Authorization: Bearer <value>` header is present but `<value>` does not start with `erb_`
- **THEN** the backend does not consult `api_key` and falls back to the session cookie

### Requirement: POST /api/v1/keys creates a new key
`POST /api/v1/keys` SHALL be authenticated (session or bearer). It SHALL accept a JSON body of `{ "name": "<string>", "expires_at": "<RFC3339 timestamp>" | null }`. On success it SHALL generate a new key matching the format, insert a row with `scope = 'account'` and `account_id` set to the caller's account, and return `201 Created` with `{ "id": "<uuid>", "key": "<plaintext erb_…>", "name": "<name>", "expires_at": "<iso8601 | null>", "created_at": "<iso8601>" }`. The plaintext `key` field SHALL appear in this response and nowhere else.

#### Scenario: Account creates a key
- **WHEN** an authenticated account-scoped caller `POST /api/v1/keys` with `{ "name": "ci", "expires_at": null }`
- **THEN** the response is `201` with a new `id` and a plaintext `key` matching the `erb_…` format; a row exists in `api_key` with `scope = 'account'`, `account_id` set, and `key_hash = SHA-256(returned_key)` hex

#### Scenario: Plaintext key returned only once
- **WHEN** the same key is fetched via `GET /api/v1/keys`
- **THEN** the response includes the key's metadata (`id`, `name`, `scope`, `expires_at`, `created_at`) but does NOT include the plaintext `key`

#### Scenario: Server-scoped caller cannot create account-scoped keys
- **WHEN** a server-scoped caller hits `POST /api/v1/keys`
- **THEN** the request is rejected with HTTP 403 (as per the general server-scope rejection rule)

#### Scenario: Missing name is rejected
- **WHEN** `POST /api/v1/keys` is called with a body missing `name` or with an empty `name`
- **THEN** the response is HTTP 400

#### Scenario: Duplicate name is rejected
- **WHEN** `POST /api/v1/keys` is called where the `name` already exists for the account OR for the server
- **THEN** the response is HTTP 409

### Requirement: GET /api/v1/keys lists the caller's keys
`GET /api/v1/keys` SHALL be authenticated and SHALL return a JSON array of the keys belonging to the caller's `account_id`. Each element SHALL contain `id`, `name`, `scope`, `expires_at`, `created_at`. The response SHALL NOT include `key_hash` or any plaintext.

#### Scenario: List returns only the caller's keys
- **WHEN** account A calls `GET /api/v1/keys` and account B has its own keys
- **THEN** the response lists only A's keys

#### Scenario: List omits sensitive fields
- **WHEN** `GET /api/v1/keys` returns rows
- **THEN** no element contains a `key_hash` field or a plaintext key

### Requirement: DELETE /api/v1/keys/:id revokes a key
`DELETE /api/v1/keys/:id` SHALL be authenticated. It SHALL delete the matching row only if it belongs to the caller's account, returning HTTP 204 on success. If the row exists but belongs to another account, the response SHALL be HTTP 404 (do not disclose existence). If no row matches, the response SHALL be HTTP 404.

#### Scenario: Owner revokes their own key
- **WHEN** the key's owner calls `DELETE /api/v1/keys/:id`
- **THEN** the row is deleted and the response is HTTP 204

#### Scenario: Revoked key is rejected on next request
- **WHEN** a request presents a key immediately after that key has been deleted
- **THEN** the backend responds with HTTP 401

#### Scenario: Non-owner cannot revoke another account's key
- **WHEN** account A calls `DELETE /api/v1/keys/:id` where the row's `account_id` is B
- **THEN** the row is NOT deleted and the response is HTTP 404

#### Scenario: Server-scoped keys cannot be revoked via the HTTP API
- **WHEN** any caller attempts to `DELETE /api/v1/keys/:id` for a row with `scope = 'server'`
- **THEN** the response is HTTP 404 (server-scoped keys are managed out-of-band in this change)
