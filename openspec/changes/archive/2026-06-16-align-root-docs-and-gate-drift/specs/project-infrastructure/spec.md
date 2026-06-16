## MODIFIED Requirements

### Requirement: Environment variables configure all secrets and URLs
All runtime configuration SHALL be supplied via environment variables. No secrets SHALL have default values. The repository SHALL include a `.env.example` file documenting all required variables with placeholder values and comments describing how to generate them.

Required variables:
- `APP_URL` — base URL used to construct the OAuth2 redirect URI
- `ENCRYPTION_SECRET` — 32-byte hex secret for AES-256-GCM (refresh tokens + session cookie payload) and HS256 (session cookie JWT) key derivation
- `ESI_CLIENT_ID` — EVE SSO application client ID
- `ESI_CLIENT_SECRET` — EVE SSO application client secret
- `DATABASE_URL` — Postgres connection string (`postgres://user:pass@host:port/dbname`) used by the backend
- `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` — consumed by the Postgres container and referenced from `DATABASE_URL`

Optional variables:
- `ESI_CALLBACK_URL` — full OAuth2 callback URL. When unset it defaults to `{APP_URL}/auth/callback`. It exists so a deployment whose public callback path differs from `{APP_URL}/auth/callback` (e.g. behind a path-rewriting proxy) can override it. Being an optional URL with a derived default, it is not a secret and is exempt from the "no secrets SHALL have default values" rule.

#### Scenario: Missing APP_URL causes startup failure
- **WHEN** the backend starts without `APP_URL` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ENCRYPTION_SECRET causes startup failure
- **WHEN** the backend starts without `ENCRYPTION_SECRET` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ESI_CLIENT_ID causes startup failure
- **WHEN** the backend starts without `ESI_CLIENT_ID` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: Missing ESI_CLIENT_SECRET causes startup failure
- **WHEN** the backend starts without `ESI_CLIENT_SECRET` set
- **THEN** the process exits with a non-zero status and a clear error message

#### Scenario: ESI_CALLBACK_URL is optional and defaults
- **WHEN** the backend starts without `ESI_CALLBACK_URL` set
- **THEN** startup succeeds and the OAuth2 callback URL resolves to `{APP_URL}/auth/callback`

#### Scenario: .env.example documents all variables
- **WHEN** `.env.example` is read
- **THEN** all required environment variables are present with placeholder values and explanatory comments, and `ESI_CALLBACK_URL` is present, commented as optional, with its default documented
