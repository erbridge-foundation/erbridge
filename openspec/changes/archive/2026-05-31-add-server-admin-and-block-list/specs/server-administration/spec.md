## ADDED Requirements

### Requirement: AdminAccount extractor gates admin endpoints (cookie-only, fail-closed)

The system SHALL provide an `AdminAccount(Uuid)` Axum extractor that resolves the authenticated account ID **only** when that account has `is_server_admin = TRUE`. The extractor SHALL authenticate via the session cookie only; it SHALL NOT accept `Authorization: Bearer erb_…` API-key authentication. A leaked API key must never confer admin power.

The extractor SHALL reject an unauthenticated request with HTTP 401 (`unauthenticated`) and an authenticated non-admin request with HTTP 403 (`forbidden_admin_required`).

Every route registered under `/api/v1/admin/*` SHALL extract `AdminAccount`. A coverage test SHALL assert this for every registered admin route, so that a handler which omits the extractor (and is therefore ungated) fails the test — mirroring the existing `/api/v1/*` authentication coverage test.

#### Scenario: Admin endpoint with no credentials is rejected
- **WHEN** a request to any `/api/v1/admin/*` route arrives with no session cookie
- **THEN** the response is HTTP 401 with `error.code = "unauthenticated"`

#### Scenario: Admin endpoint with a non-admin session is rejected
- **WHEN** an authenticated non-admin account requests any `/api/v1/admin/*` route via session cookie
- **THEN** the response is HTTP 403 with `error.code = "forbidden_admin_required"`

#### Scenario: Admin endpoint rejects API-key authentication
- **WHEN** a request to any `/api/v1/admin/*` route presents a valid account-scoped `Authorization: Bearer erb_…` key whose account IS a server admin
- **THEN** the response is HTTP 401 (the admin extractor does not consult API keys); admin actions require a session cookie

#### Scenario: Admin endpoint with an admin session proceeds
- **WHEN** a server-admin account requests an `/api/v1/admin/*` route via a valid session cookie
- **THEN** the extractor resolves the admin's account ID and the handler proceeds

#### Scenario: Every admin route is gated
- **WHEN** the admin-route coverage test enumerates every registered `/api/v1/admin/*` route
- **THEN** each route's handler extracts `AdminAccount`; a route that does not fails the test

### Requirement: GET /api/v1/admin/accounts lists all accounts

`GET /api/v1/admin/accounts` SHALL return every account on the instance (newest first), each with its `id`, `status`, `is_server_admin`, `created_at`, and the account's characters (at least `eve_character_id`, `name`, `is_main`) so the admin UI can identify accounts by character. The response conforms to the `api-contract` success envelope.

#### Scenario: Admin lists accounts
- **WHEN** a server admin calls `GET /api/v1/admin/accounts`
- **THEN** the response is `200` with `data` containing every account and, for each, its characters

### Requirement: GET /api/v1/admin/characters/search resolves a name fragment to accounts

`GET /api/v1/admin/characters/search?q=<fragment>` SHALL return characters whose name matches the fragment (case-insensitive substring), each with its `eve_character_id`, `name`, `is_main`, and owning `account_id`, so the admin UI can resolve "promote the account that owns *Pilot X*" without enumerating every account. The query SHALL bind `q` as a parameter (no SQL injection surface) and SHALL cap the number of returned rows.

#### Scenario: Search returns matching characters with their account
- **WHEN** a server admin calls `GET /api/v1/admin/characters/search?q=pil`
- **THEN** the response is `200` with `data` listing characters whose name contains "pil" (case-insensitive), each carrying its owning `account_id`

#### Scenario: Search result cap
- **WHEN** a search matches more characters than the cap
- **THEN** the response returns at most the cap and does not error

### Requirement: POST /api/v1/admin/accounts/:id/grant-admin grants server admin

`POST /api/v1/admin/accounts/:id/grant-admin` SHALL set `is_server_admin = TRUE` on the target account. It SHALL be idempotent: granting an account that is already an admin SHALL return success (HTTP 204) and SHALL NOT emit an audit event. Granting a non-existent account SHALL return HTTP 404. A successful (state-changing) grant SHALL emit `ServerAdminGranted { account_id, source: AdminGrant }` in the same transaction.

#### Scenario: Admin grants admin to another account
- **WHEN** a server admin calls `POST /api/v1/admin/accounts/<id>/grant-admin` for a non-admin account
- **THEN** the account's `is_server_admin` becomes `TRUE`, the response is HTTP 204, and an `audit_log` row exists with `event_type = "server_admin_granted"` and `details.source = "admin_grant"`

#### Scenario: Granting an already-admin account is an idempotent no-op
- **WHEN** a server admin calls grant-admin for an account that is already an admin
- **THEN** the response is HTTP 204 and no new `audit_log` row is written

#### Scenario: Granting a non-existent account
- **WHEN** grant-admin targets an account id that does not exist
- **THEN** the response is HTTP 404

### Requirement: POST /api/v1/admin/accounts/:id/revoke-admin revokes server admin with a last-admin guard

`POST /api/v1/admin/accounts/:id/revoke-admin` SHALL clear `is_server_admin` on the target account. The last-admin guard SHALL run inside the transaction: if revoking would reduce the count of active server admins to zero, the request SHALL be rejected with HTTP 409 (`cannot_remove_last_server_admin`) and the transaction rolled back. Revoking an account that is not an admin SHALL be an idempotent no-op (HTTP 204, no audit event). Revoking a non-existent account SHALL return HTTP 404. Self-revoke SHALL be permitted as long as the last-admin guard holds. A successful (state-changing) revoke SHALL emit `ServerAdminRevoked { account_id }` in the same transaction.

#### Scenario: Admin revokes another admin
- **GIVEN** two server admins exist
- **WHEN** one calls `POST /api/v1/admin/accounts/<other>/revoke-admin`
- **THEN** the other's `is_server_admin` becomes `FALSE`, the response is HTTP 204, and an `audit_log` row with `event_type = "server_admin_revoked"` exists

#### Scenario: Cannot revoke the last admin
- **GIVEN** exactly one active server admin
- **WHEN** any admin calls revoke-admin targeting that last admin (including self)
- **THEN** the response is HTTP 409 with `error.code = "cannot_remove_last_server_admin"` and `is_server_admin` is unchanged

#### Scenario: Revoking a non-admin account is an idempotent no-op
- **WHEN** revoke-admin targets an account that is not an admin
- **THEN** the response is HTTP 204 and no `audit_log` row is written

### Requirement: blocked_eve_character table is a self-contained snapshot

The system SHALL provide a `blocked_eve_character` table with columns:

- `eve_character_id` — `BIGINT PRIMARY KEY`
- `character_name` — `TEXT` (nullable; snapshot at block time)
- `corporation_name` — `TEXT` (nullable; snapshot at block time)
- `reason` — `TEXT` (nullable)
- `blocked_by` — `UUID REFERENCES account(id) ON DELETE SET NULL`
- `blocked_at` — `TIMESTAMPTZ NOT NULL DEFAULT now()`

The table SHALL NOT have a foreign key to `eve_character`. The row is a self-contained snapshot so that a character who has never signed in (no `eve_character` row) can be blocked pre-emptively, and so the block list reads without joining `eve_character`.

The `character_name` / `corporation_name` snapshot SHALL be populated best-effort from ESI public-info at block time. Because CCP does not permit player-initiated character renames and the EVE character ID is immutable, the snapshot is effectively permanent-correct.

#### Scenario: Schema is created by migration
- **WHEN** the backend applies all migrations
- **THEN** the `blocked_eve_character` table exists with the six columns above and no FK to `eve_character`

#### Scenario: An unknown character can be blocked
- **WHEN** an admin blocks an `eve_character_id` for which no `eve_character` row exists
- **THEN** the block row is inserted successfully (no FK violation)

### Requirement: POST /api/v1/admin/blocks blocks a character and tears down the owning account

`POST /api/v1/admin/blocks` SHALL accept `{ "eve_character_id": <bigint>, "reason": "<string>" | null }` and insert a `blocked_eve_character` row. It SHALL be idempotent: blocking an already-blocked character SHALL return success (HTTP 204) and SHALL NOT emit an audit event.

The endpoint SHALL fetch ESI public-info best-effort to populate `character_name` / `corporation_name`. The block SHALL succeed even when ESI is unavailable, inserting the row with null name/corp; enforcement keys on the immutable `eve_character_id`, not the name.

An admin SHALL NOT block any character belonging to their own account. Such a request SHALL be rejected with HTTP 409 (`cannot_block_self`) and SHALL write nothing.

When the blocked character currently resolves to an account (i.e. an `eve_character` row exists with a non-null `account_id`), the same transaction SHALL, for that account: clear the EVE-credential columns of every owned `eve_character` row (`encrypted_access_token = NULL`, `encrypted_refresh_token = NULL`, `access_token_expires_at = NULL`, `scopes = '{}'`) and delete every row in the `session` table belonging to it. A successful (state-changing) block SHALL emit `EveCharacterBlocked { eve_character_id, reason }` in the same transaction.

#### Scenario: Block a character belonging to an account tears the account down
- **GIVEN** an account A owns characters X (blocked target) and Y, both with tokens and an active session
- **WHEN** a server admin blocks X's `eve_character_id`
- **THEN** the block row is inserted; in the same transaction every owned character (X and Y) has its EVE-credential columns cleared and all of A's sessions are deleted; an `audit_log` row with `event_type = "eve_character_blocked"` exists; the response is HTTP 204

#### Scenario: Block an unknown character records the block with no teardown
- **WHEN** a server admin blocks an `eve_character_id` that resolves to no account
- **THEN** the block row is inserted; no tokens or sessions are touched (there is no owning account); the response is HTTP 204

#### Scenario: Block succeeds when ESI is unavailable
- **WHEN** ESI public-info is unreachable at block time
- **THEN** the block row is inserted with null `character_name` / `corporation_name` and the response is HTTP 204; the block is fully effective

#### Scenario: Admin cannot block their own character
- **WHEN** a server admin blocks an `eve_character_id` that belongs to their own account
- **THEN** the response is HTTP 409 with `error.code = "cannot_block_self"` and no block row, token clear, or session deletion occurs

#### Scenario: Blocking an already-blocked character is an idempotent no-op
- **WHEN** a server admin blocks a character that is already in the block list
- **THEN** the response is HTTP 204 and no new `audit_log` row is written

### Requirement: DELETE /api/v1/admin/blocks/:eve_character_id unblocks a character

`DELETE /api/v1/admin/blocks/:eve_character_id` SHALL remove the `blocked_eve_character` row. If no row matches, the response SHALL be HTTP 404. A successful unblock SHALL emit `EveCharacterUnblocked { eve_character_id }` in the same transaction. Unblock SHALL NOT restore tokens or sessions — the formerly-blocked account's characters remain `token_status = "expired"` until each is re-authorised via SSO.

#### Scenario: Admin unblocks a character
- **WHEN** a server admin calls `DELETE /api/v1/admin/blocks/<eve_character_id>` for a blocked character
- **THEN** the block row is removed, an `audit_log` row with `event_type = "eve_character_unblocked"` exists, and the response is HTTP 204

#### Scenario: Unblocking a non-blocked character
- **WHEN** `DELETE /api/v1/admin/blocks/:eve_character_id` targets an id not in the block list
- **THEN** the response is HTTP 404

### Requirement: GET /api/v1/admin/blocks lists blocked characters

`GET /api/v1/admin/blocks` SHALL return every `blocked_eve_character` row (newest first) with `eve_character_id`, `character_name`, `corporation_name`, `reason`, `blocked_by`, and `blocked_at`. The list SHALL read without joining `eve_character`.

#### Scenario: Admin lists blocks
- **WHEN** a server admin calls `GET /api/v1/admin/blocks`
- **THEN** the response is `200` with `data` listing every blocked character and its snapshot fields

### Requirement: An account is blocked iff it owns a blocked character (derived)

An account SHALL be considered blocked if and only if it owns at least one `eve_character` whose `eve_character_id` is present in `blocked_eve_character`. There SHALL be no separate per-account "blocked" flag; the state is derived from the block list. Adding a character to an account that owns a blocked character does not change this; removing the last blocked character (via unblock) makes the account no longer blocked.

#### Scenario: Account with one blocked character is blocked
- **GIVEN** an account owning characters X and Y where X is in the block list
- **WHEN** the account's blocked status is evaluated
- **THEN** the account is blocked

#### Scenario: Unblocking the only blocked character unblocks the account
- **GIVEN** an account blocked solely because character X is blocked
- **WHEN** X is unblocked
- **THEN** the account is no longer blocked

### Requirement: Block enforcement mirrors soft-delete — no hot-path check

Block enforcement SHALL rely on session teardown plus checks at the two surviving authentication routes, exactly as soft-delete is enforced:

- The block action deletes all of the owning account's sessions (above), so the session-cookie path cannot present a live session for a blocked account. The session-cookie branch of `AuthenticatedAccount` SHALL NOT perform a block-list check (the absence of a session is the enforcement, identical to soft-delete).
- The bearer branch of `AuthenticatedAccount` (the route that survives session teardown, since API keys are not deleted on block) SHALL reject a request whose account owns a blocked character, via a join against `blocked_eve_character`, with HTTP 401 (`account_blocked`). This sits alongside the existing `account_soft_deleted` check.
- The SSO callback SHALL reject a login whose resolved `eve_character_id` is blocked (per the `eve-sso-auth` capability), so a blocked account cannot obtain a new session.

#### Scenario: Blocked account's cookie request fails because the session is gone
- **GIVEN** an account blocked while it had an active session
- **WHEN** a request arrives with that account's (now-deleted) session cookie
- **THEN** the response is HTTP 401 `unauthenticated` (the session no longer exists); the cookie branch performs no block-list query

#### Scenario: Blocked account's bearer request is rejected
- **WHEN** a request presents a valid account-scoped API key whose account owns a blocked character
- **THEN** the bearer branch rejects it with HTTP 401 and `error.code = "account_blocked"`; the key row is not deleted

#### Scenario: Hot cookie path is not taxed by blocking
- **WHEN** a non-blocked account makes a session-cookie request (e.g. `GET /api/v1/me`)
- **THEN** the request is served without any query against `blocked_eve_character`

### Requirement: Long-lived authenticated connections re-validate the session

Any long-lived authenticated connection (e.g. a future Server-Sent Events or websocket endpoint) SHALL periodically re-validate its session against the `session` table for the duration of the connection. Such a connection SHALL NOT authenticate once at connection-open and then stream indefinitely without re-validation.

Because block (like soft-delete and logout) deletes the session row, a connection that re-validates SHALL discover the deletion within one re-validation interval and close. The client's automatic reconnect re-runs `AuthenticatedAccount` (the session-cookie path finds no session and returns HTTP 401; the block additionally prevents establishing a fresh session via SSO). A reconnecting client that is blocked SHALL be routed to the blocked-information page rather than the login page, using the distinct `account_blocked` error.

The re-validation SHOULD reuse the existing sliding-expiry mechanism: the same heartbeat that refreshes `last_seen_at` / `expires_at` is the moment the connection observes a deleted session. A separate block-polling path SHALL NOT be required.

#### Scenario: A blocked pilot's open stream terminates within one heartbeat
- **GIVEN** a long-lived authenticated connection that re-validates its session on a heartbeat
- **WHEN** the connection's account is blocked (its session row is deleted)
- **THEN** the next heartbeat finds no session and the connection closes; on automatic reconnect the request is rejected (session-cookie path → 401), bounding block latency to one heartbeat interval

#### Scenario: Stream-forever is not permitted
- **WHEN** a long-lived authenticated endpoint is designed
- **THEN** it re-validates the session periodically; an endpoint that authenticates once and never re-validates does not satisfy this requirement

### Requirement: GET /api/v1/admin/audit exposes the audit log to admins

`GET /api/v1/admin/audit` SHALL return audit-log entries newest-first via `audit::list_audit_log` (per the `audit-log` capability). It SHALL accept optional `event_type`, `actor` (account UUID), `target_type`, `target_id`, and `target_name` (case-insensitive substring match) query parameters, and a `before` (RFC 3339 keyset cursor), and a `limit` clamped to a sensible maximum (defaulting when omitted). The target filters realise the dominant target-first admin query ("who did X to whom"); `target_name` is the axis a human searches on, so it matches a substring fragment (e.g. `wasp` finds `Wasp 223`) rather than requiring the full name. All supplied filters combine conjunctively. The response SHALL include the entries and a `next_before` cursor (the oldest returned entry's `occurred_at`) for pagination. Each entry exposes `id`, `occurred_at`, `actor_account_id`, `actor_character_id`, `actor_character_name`, `event_type`, `details`, `target_type`, `target_id`, and `target_name`.

#### Scenario: Admin reads the audit log
- **WHEN** a server admin calls `GET /api/v1/admin/audit`
- **THEN** the response is `200` with `data.entries` newest-first (capped at the clamped limit) and a `next_before` cursor; each entry carries its `target_type`/`target_id`/`target_name`

#### Scenario: Admin filters and paginates
- **WHEN** a server admin calls `GET /api/v1/admin/audit?event_type=eve_character_blocked&before=<ts>&limit=20`
- **THEN** only `eve_character_blocked` entries older than `<ts>` are returned, at most 20, newest-first

#### Scenario: Admin searches target-first by name fragment
- **WHEN** a server admin calls `GET /api/v1/admin/audit?target_name=wasp`
- **THEN** only entries whose `target_name` contains "wasp" case-insensitively (e.g. "Wasp 223") are returned, newest-first — answering "what was done to this pilot/account" regardless of who the actor was, without the admin needing to type the full name

### Requirement: Admin frontend is gated and surfaced only to admins

The frontend SHALL provide an `/admin` route group. Its server-side load SHALL respond with HTTP 404 for any caller that is not a server admin (the existence of admin pages is not disclosed). The route group SHALL include an overview (`/admin`), admin management (`/admin/admins`), block management (`/admin/blocks`), and the audit browser (`/admin/audit`). The global navigation SHALL surface an "Admin" affordance only when the authenticated account's `is_server_admin` (from `GET /api/v1/me`) is `true`. The frontend SHALL provide a `/blocked` information page shown to a blocked pilot whose request is rejected with `account_blocked`.

#### Scenario: Non-admin cannot reach /admin
- **WHEN** a non-admin (or unauthenticated) user navigates to any `/admin` route
- **THEN** the server-side load returns HTTP 404; the page's existence is not disclosed

#### Scenario: Admin link shown only to admins
- **WHEN** the global navigation renders for a non-admin account
- **THEN** no "Admin" affordance is present
- **WHEN** it renders for a server-admin account
- **THEN** an "Admin" affordance linking to `/admin` is present

#### Scenario: Admin promotes by character search
- **WHEN** an admin uses `/admin/admins` to search for a character by name and confirms promotion of the owning account
- **THEN** the frontend resolves the character to its `account_id` and submits grant-admin for that account
