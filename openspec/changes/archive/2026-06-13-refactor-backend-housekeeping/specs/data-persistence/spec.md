# data-persistence — delta for refactor-backend-housekeeping

## MODIFIED Requirements

### Requirement: Session table
The `session` table SHALL match:

```sql
CREATE TABLE session (
    session_id         TEXT        PRIMARY KEY,
    account_id         UUID        NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at         TIMESTAMPTZ NOT NULL
);

CREATE INDEX session_expires_at_idx ON session (expires_at);
CREATE INDEX session_account_id_idx ON session (account_id);
```

- `session_id` is the opaque session identifier carried inside the session cookie's JWT. The JWT signature establishes integrity; the `session_id` is the lookup key into this table.
- `account_id` is the authenticated account the session resolves to. `ON DELETE CASCADE` so account hard-deletion sweeps the account's sessions.
- The historical `csrf_state` and `add_character_mode` columns (carried from the in-flight OAuth2 record at session creation by earlier iterations) are dropped by migration: nothing reads them — CSRF state lives entirely in the pre-session in-flight record, and by the time a session row exists the OAuth dance is complete.
- `created_at` records first sight; `last_seen_at` is advanced on every authenticated request that resolves via this row; `expires_at` is the moment past which the row is treated as if it does not exist.
- The `session_expires_at_idx` index supports the expired-session reaping requirement below; `session_account_id_idx` supports `list_session_ids_for_account` (used by `DELETE /api/v1/account` to drop every session belonging to a soft-deleted account).

#### Scenario: Session rows survive backend restart
- **WHEN** a session row is created, the backend process is restarted, and a browser presents the same session cookie before `expires_at`
- **THEN** the row is still present and the request is authenticated against it; no re-login is required

#### Scenario: Session rows hold no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold access tokens, refresh tokens, or any other ESI credentials

#### Scenario: Account deletion cascades to sessions
- **WHEN** an `account` row is deleted
- **THEN** all `session` rows where `account_id` matches are removed via `ON DELETE CASCADE`

## ADDED Requirements

### Requirement: Expired session rows are reaped

The system SHALL delete `session` rows whose `expires_at` has passed on a recurring schedule, at least daily. Expired rows are already invisible to authentication (the lookup predicate excludes them); reaping bounds table growth. The reap SHALL log the number of rows removed.

#### Scenario: Expired rows are removed by the recurring reap

- **WHEN** the recurring reap runs while the table contains rows with `expires_at < now()` and rows with `expires_at > now()`
- **THEN** the expired rows are deleted and the live rows are untouched

#### Scenario: Reaping is hygiene, not enforcement

- **WHEN** a request presents a session whose row is expired but not yet reaped
- **THEN** the request is rejected exactly as if the row were absent (reaping latency has no authentication effect)
