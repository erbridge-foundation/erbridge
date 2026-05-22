## Purpose

Postgres 16 schema for accounts, EVE characters, API keys, and sessions; SQL migration framework run at backend startup; and encrypted-at-rest storage of ESI tokens. Establishes the durable storage substrate for identity in the system.

## Requirements

### Requirement: Postgres is the system of record for identity
The backend SHALL use Postgres 16 (or newer) as the durable store for the `account` and `eve_character` tables (table names are singular by project convention). The database SHALL be reachable via a `DATABASE_URL` environment variable in the standard `postgres://user:pass@host:port/dbname` format. The backend SHALL connect using a pooled `sqlx::PgPool`.

#### Scenario: Backend connects to Postgres at startup
- **WHEN** the backend starts with a valid `DATABASE_URL`
- **THEN** a `PgPool` is created, an initial connection succeeds, and the pool is stored in `AppState`

#### Scenario: Missing DATABASE_URL causes startup failure
- **WHEN** the backend starts without `DATABASE_URL` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Unreachable Postgres at startup
- **WHEN** the backend starts and the configured Postgres is not reachable
- **THEN** the process retries the initial connection for a bounded period and then exits with a non-zero status if still unreachable

### Requirement: Schema is managed by SQL migrations
The repository SHALL contain a `backend/migrations/` directory of timestamp-prefixed SQL files (e.g. `20260516120000_create_account_and_eve_character.sql`). Migrations SHALL be applied at backend startup via `sqlx::migrate!`. The backend SHALL exit with a clear error if a migration fails. All table identifiers introduced by any migration SHALL be singular nouns (e.g. `account`, `eve_character`, never `accounts` or `eve_characters`).

#### Scenario: Migrations run on startup
- **WHEN** the backend starts against an empty database
- **THEN** all migration files in `backend/migrations/` are applied in order before the HTTP server begins accepting connections

#### Scenario: Migration failure prevents startup
- **WHEN** a migration fails to apply (syntax error, conflicting state, etc.)
- **THEN** the backend exits with a non-zero status and the error is logged

#### Scenario: Idempotent re-run
- **WHEN** the backend starts against a database that already has all migrations applied
- **THEN** no migration is re-run and startup proceeds normally

#### Scenario: Table names are singular
- **WHEN** the schema is inspected after migrations run
- **THEN** every table name is a singular noun

### Requirement: Account table
The initial migration SHALL create an `account` table matching:

```sql
CREATE TABLE account (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    status              TEXT        NOT NULL DEFAULT 'active',
    delete_requested_at TIMESTAMPTZ,
    is_server_admin     BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX account_server_admin_idx ON account (id) WHERE is_server_admin = TRUE;
```

- `status` is text. Values used by this change: `'active'` (default) and `'soft_deleted'`. The column is intentionally open-ended to accommodate future moderation statuses (`'banned'`, `'suspended'`) without a schema change. Moderation statuses are set account-wide via a per-character action (specified in a future change).
- `delete_requested_at` is set when a user soft-deletes their account; the hard-delete cooldown sweeper is a future change.
- `is_server_admin` flags accounts with administrative privileges across the deployment. The partial index keeps admin lookups fast.

An `account` row SHALL be created the first time a previously-unseen EVE character completes the OAuth2 login flow.

#### Scenario: First login creates an account
- **WHEN** a user completes the OAuth2 callback with an `eve_character_id` not present in `eve_character` (or present only as an orphan)
- **THEN** a new row is inserted into `account` with `status = 'active'`; the resulting `eve_character` row references its `id`

#### Scenario: Repeat login for an existing character does not create a new account
- **WHEN** an EVE character already linked to an `'active'` account completes the OAuth2 callback (not via add-character)
- **THEN** no new `account` row is created; the session is associated with the existing account

#### Scenario: Soft-delete sets status and timestamp
- **WHEN** an authenticated user requests account deletion
- **THEN** the `account` row's `status` is set to `'soft_deleted'`, `delete_requested_at` is set to `now()`, and all active sessions for that account are removed from the session store

#### Scenario: Login reactivates a soft-deleted account
- **WHEN** an SSO login completes for a character whose `account.status` is `'soft_deleted'`
- **THEN** the account row is updated atomically with the character upsert to set `status = 'active'` and `delete_requested_at = NULL`

#### Scenario: Server admin lookup uses the partial index
- **WHEN** the backend queries `SELECT id FROM account WHERE is_server_admin = TRUE`
- **THEN** Postgres uses `account_server_admin_idx` (verifiable via `EXPLAIN`)

### Requirement: EVE character table
The initial migration SHALL create an `eve_character` table matching:

```sql
CREATE TABLE eve_character (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id              UUID        REFERENCES account(id) ON DELETE CASCADE,
    eve_character_id        BIGINT      NOT NULL UNIQUE,
    name                    TEXT        NOT NULL,
    corporation_id          BIGINT      NOT NULL,
    corporation_name        TEXT        NOT NULL,
    alliance_id             BIGINT,
    alliance_name           TEXT,
    is_main                 BOOLEAN     NOT NULL DEFAULT false,
    is_online               BOOLEAN,
    esi_client_id           TEXT,
    encrypted_access_token  BYTEA,
    encrypted_refresh_token BYTEA,
    access_token_expires_at TIMESTAMPTZ,
    scopes                  TEXT[]      NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX eve_character_one_main_per_account
    ON eve_character(account_id)
    WHERE is_main = true;

CREATE INDEX eve_character_account_id_idx ON eve_character (account_id);
```

- `account_id` is NULLABLE. A NULL value means the row is an **orphan**: a public-info cache populated by flows like map-ACL pre-claim, before any pilot has signed in as this character. Such rows have NULL token columns.
- `eve_character_id` is the BIGINT EVE character ID from ESI; `UNIQUE` prevents duplicates.
- `name`, `corporation_id`, `corporation_name`, `alliance_id`, `alliance_name` mirror ESI public info **at the time the row was last written**. `corporation_id` and `corporation_name` are NOT NULL (all EVE characters belong to a corp); `alliance_id` and `alliance_name` are NULL together when the corp is not in an alliance. These columns are **denormalised** so that `GET /api/v1/me` is a pure DB read; they are refreshed on every SSO callback (login, add-character, re-auth) and by a future background job that polls active accounts, neither of which is on the request hot path.
- `is_main` marks one character per account as primary. The partial unique index permits any number of `false`/NULL rows but at most one `true` per `account_id`.
- `is_online` mirrors `esi-location.read_online.v1`; NULL until first poll.
- `esi_client_id` records which ESI client ID issued the stored tokens (for future credential rotation).
- `encrypted_access_token` and `encrypted_refresh_token` are AES-256-GCM ciphertexts (nonce + ciphertext + auth tag); both NULL for orphans.
- `access_token_expires_at` records the **access** token's expiry (typically ~20 minutes after issue). It is an implementation detail used by a future refresh-on-demand flow to decide whether to refresh before the next ESI call; it is NOT the refresh token's expiry, and it does NOT determine `token_status` in `GET /api/v1/me` (per account-management).
- `scopes` is the array of ESI scope identifiers the user granted during SSO, parsed from the access-token JWT's `scp` claim. Stored as a `TEXT[]` (Postgres array) so subset checks (`required ⊆ granted`) are a single SQL operation when a future change introduces required-scope-set drift detection. `NOT NULL DEFAULT '{}'` — orphan rows have an empty array, which correctly means "no scopes granted yet". The current foundation change does not read this column from any handler; it exists so the future capability that does (e.g. detecting `missing_scopes` for `token_status`) does not require a migration.

#### Scenario: New character linked to a session adds a row
- **WHEN** a user completes a login or add-character flow with an `eve_character_id` not present in `eve_character`
- **THEN** a new `eve_character` row is inserted with `account_id` set to the session's account, tokens encrypted, `name` / `corporation_id` / `alliance_id` populated from ESI

#### Scenario: Re-login overwrites tokens on an existing row
- **WHEN** a user completes any login flow with an `eve_character_id` already present in `eve_character` whose `account_id` matches the session's account (or is NULL — orphan claim)
- **THEN** the existing row is updated: `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, `scopes`, refreshed `name` / `corporation_id` / `corporation_name` / `alliance_id` / `alliance_name`, and `updated_at`; `account_id` is set to the session's account if it was previously NULL

#### Scenario: Orphan character is claimed on first login
- **WHEN** an SSO login completes for an `eve_character_id` that already exists with `account_id = NULL`
- **THEN** that row's `account_id` is set to the logging-in user's account, tokens are written, and no second row is created

#### Scenario: At most one main per account
- **WHEN** an attempt is made to set `is_main = true` on a second character of an account that already has a main
- **THEN** the operation MUST first clear `is_main` on the existing main in the same transaction, or the partial unique index `eve_character_one_main_per_account` MUST reject the write

#### Scenario: Removing a character hard-deletes the row
- **WHEN** a user removes a character from their account
- **THEN** the `eve_character` row is `DELETE`d; re-adding the same character later creates a fresh row (new `id`)

#### Scenario: Account hard-delete cascades to characters
- **WHEN** an `account` row is deleted (by a future sweeper or by an admin)
- **THEN** all `eve_character` rows where `account_id` matches are removed via `ON DELETE CASCADE`

### Requirement: API key table
The initial migration SHALL create an `api_key` table matching:

```sql
CREATE TABLE api_key (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    scope         TEXT        NOT NULL,
    account_id    UUID        REFERENCES account(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    key_hash      TEXT        NOT NULL,
    expires_at    TIMESTAMPTZ NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT api_key_scope_check CHECK (
        (scope = 'account' AND account_id IS NOT NULL)
        OR (scope = 'server' AND account_id IS NULL)
    )
);

CREATE UNIQUE INDEX api_key_hash_idx ON api_key (key_hash);
CREATE INDEX api_key_account_idx ON api_key (account_id) WHERE account_id IS NOT NULL;
```

- `scope` SHALL be `'account'` or `'server'`. The CHECK constraint enforces `account_id IS NOT NULL` for `'account'` keys and `account_id IS NULL` for `'server'` keys.
- `key_hash` is `SHA-256(plaintext_key)` as a lowercase hex string. Plaintext keys SHALL NOT appear in the database, in logs, or in any cache.
- `expires_at` NULL means the key has no expiry; otherwise the key is rejected once `now() > expires_at`.
- Revocation is hard `DELETE` of the row — there is no `revoked_at` tombstone.
- `account_id` uses `ON DELETE CASCADE` so account hard-deletion sweeps the account's keys.

#### Scenario: Account-scoped key requires account_id
- **WHEN** an `INSERT` attempts `scope = 'account'` with `account_id = NULL`
- **THEN** the `api_key_scope_check` constraint rejects the write

#### Scenario: Server-scoped key requires account_id to be NULL
- **WHEN** an `INSERT` attempts `scope = 'server'` with a non-NULL `account_id`
- **THEN** the `api_key_scope_check` constraint rejects the write

#### Scenario: key_hash is unique
- **WHEN** an `INSERT` attempts to store a row with a `key_hash` that already exists
- **THEN** the unique index `api_key_hash_idx` rejects the write

#### Scenario: Account deletion cascades to keys
- **WHEN** an `account` row is deleted
- **THEN** all `api_key` rows where `account_id` matches are removed

#### Scenario: Plaintext key never appears in the database
- **WHEN** a row is inspected after creation
- **THEN** no column contains the plaintext key; only the SHA-256 hex digest in `key_hash`

### Requirement: Session table
The initial migration SHALL create a `session` table matching:

```sql
CREATE TABLE session (
    session_id         TEXT        PRIMARY KEY,
    account_id         UUID        NOT NULL REFERENCES account(id) ON DELETE CASCADE,
    csrf_state         TEXT,
    add_character_mode BOOL        NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at         TIMESTAMPTZ NOT NULL
);

CREATE INDEX session_expires_at_idx ON session (expires_at);
CREATE INDEX session_account_id_idx ON session (account_id);
```

- `session_id` is the opaque session identifier carried inside the session cookie's JWT. The JWT signature establishes integrity; the `session_id` is the lookup key into this table.
- `account_id` is the authenticated account the session resolves to. `ON DELETE CASCADE` so account hard-deletion sweeps the account's sessions.
- `csrf_state` and `add_character_mode` are carried forward from the in-flight OAuth2 record at the moment of session creation. They are persisted so a backend restart between SSO start and SSO callback does not strand the user. (The in-flight OAuth2 record itself remains in-memory by design — it has no `account_id` yet and is intentionally restart-volatile.)
- `created_at` records first sight; `last_seen_at` is advanced on every authenticated request that resolves via this row; `expires_at` is the moment past which the row is treated as if it does not exist.
- The `session_expires_at_idx` partial index supports the opportunistic `DELETE FROM session WHERE expires_at < now()` cleanup path; `session_account_id_idx` supports `list_session_ids_for_account` (used by `DELETE /api/v1/account` to drop every session belonging to a soft-deleted account).

#### Scenario: Session rows survive backend restart
- **WHEN** a session row is created, the backend process is restarted, and a browser presents the same session cookie before `expires_at`
- **THEN** the row is still present and the request is authenticated against it; no re-login is required

#### Scenario: Session rows hold no token material
- **WHEN** any row in the `session` table is inspected
- **THEN** the row holds only `session_id`, `account_id`, `csrf_state`, `add_character_mode`, `created_at`, `last_seen_at`, and `expires_at`; it does NOT hold access tokens, refresh tokens, or any other ESI credentials

#### Scenario: Account deletion cascades to sessions
- **WHEN** an `account` row is deleted
- **THEN** all `session` rows where `account_id` matches are removed via `ON DELETE CASCADE`

### Requirement: ESI tokens are encrypted at rest
Both ESI access tokens and refresh tokens SHALL be encrypted with AES-256-GCM using a key derived from `ENCRYPTION_SECRET` before being written to `eve_character.encrypted_access_token` and `eve_character.encrypted_refresh_token`. A fresh 12-byte random nonce SHALL be generated per write and stored inline with the ciphertext (e.g. nonce-prefixed). Plaintext tokens SHALL NOT appear in logs, error messages, or any other persistent store. Postgres is the single source of truth for tokens; no plaintext token copy SHALL be cached elsewhere across requests.

#### Scenario: Tokens round-trip through Postgres
- **WHEN** an access or refresh token is written and then read back
- **THEN** the decrypted plaintext matches the original; the ciphertext in the database does not equal the plaintext

#### Scenario: Plaintext tokens never appear in logs
- **WHEN** the backend logs at any level (info/debug/trace) during the OAuth2 callback flow or any ESI call
- **THEN** no log line contains the plaintext access-token or refresh-token value

#### Scenario: Orphan rows have no token material
- **WHEN** an `eve_character` row is created via an orphan flow (no associated session)
- **THEN** `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `esi_client_id`, and `account_id` are all NULL (and `scopes` is the empty array `'{}'`, the column default)

### Requirement: Postgres runs as a Compose service
The `docker-compose.yml` SHALL include a `postgres` service using the official `postgres:16` image. The service SHALL use a named volume for `/var/lib/postgresql/data` so data persists across `docker compose down`. The `backend` service SHALL depend on `postgres` being healthy before it starts.

#### Scenario: Data survives a compose restart
- **WHEN** `docker compose down` is run and then `docker compose up` is run again (without `-v`)
- **THEN** previously-created `account` and `eve_character` rows are still present

#### Scenario: Backend waits for Postgres
- **WHEN** `docker compose up --build` is run from a cold start
- **THEN** the backend container does not begin its startup sequence until Postgres reports healthy
