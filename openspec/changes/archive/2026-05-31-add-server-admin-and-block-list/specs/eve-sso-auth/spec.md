## ADDED Requirements

### Requirement: SSO callback rejects blocked characters

The OAuth2 callback handler SHALL, after resolving the `eve_character_id` from the access-token JWT and **before** any account or character write, check whether that `eve_character_id` is present in `blocked_eve_character` (per the `server-administration` capability). If it is blocked, the callback SHALL reject the login: it SHALL NOT create or modify any `account` or `eve_character` row, SHALL NOT persist tokens, SHALL NOT create or update a session, and SHALL NOT set a session cookie. The browser SHALL be redirected to the `/blocked` information page (or an equivalent blocked response).

This check SHALL apply to **both** the login flow and the add-character flow, so that a blocked pilot can neither sign in as themselves nor be attached as an alt to an existing (even unblocked) account.

The rejection SHALL emit a `BlockedLoginRejected { eve_character_id }` audit event (per the `audit-log` capability) with `actor_account_id = NULL` (no account is authenticated) and the `eve_character_id` carried in `details`.

#### Scenario: Blocked character cannot log in
- **GIVEN** an `eve_character_id` present in `blocked_eve_character`
- **WHEN** that character completes the SSO flow at `/auth/callback`
- **THEN** no `account` or `eve_character` row is created or modified, no session is established, no session cookie is set, and the browser is redirected to `/blocked`; an `audit_log` row with `event_type = "blocked_login_rejected"` and `details.eve_character_id` equal to that id exists

#### Scenario: Blocked character cannot be added as an alt
- **GIVEN** an authenticated session for an unblocked account, and a blocked `eve_character_id`
- **WHEN** the account attempts to add that blocked character via the add-character flow and SSO completes
- **THEN** the blocked character is not attached to the account, no token is persisted for it, and a `blocked_login_rejected` audit row is written

#### Scenario: Block check precedes account creation
- **WHEN** a never-before-seen blocked `eve_character_id` completes SSO
- **THEN** the block is detected before any `account` row would be created, so no orphaned account results from the rejected login
