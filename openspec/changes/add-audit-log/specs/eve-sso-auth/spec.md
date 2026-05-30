## ADDED Requirements

### Requirement: SSO callback emits audit events for account-lifecycle transitions

The OAuth2 callback handler SHALL emit audit events (per the `audit-log` capability) into the same transaction that performs each of the following actions. Emissions SHALL occur *after* the `promote_if_no_main` step so that the actor-character snapshot resolves correctly for any subsequent audit emission in the same transaction.

Concretely, for each transaction processed by `GET /auth/callback`:

1. If the callback creates a new `account` row (the first-character flow), it SHALL emit `AccountRegistered { account_id, eve_character_id, character_name }` using `acting_as = Some(ActingCharacter { eve_character_id, name: character_name })` and `actor_account_id = None` (no session exists yet).
2. If the callback claims a pre-existing orphan `eve_character` row (one with `account_id IS NULL`), it SHALL emit `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` with the same `acting_as` / `actor_account_id = None` pattern.
3. If the callback reactivates a soft-deleted account (per the existing "Login reactivates a soft-deleted account" scenario), it SHALL emit `AccountReactivated { account_id }` with `acting_as = Some(ActingCharacter { … })` and `actor_account_id = None` (the session is being re-established within this transaction).
4. If the callback promotes the just-resolved account to server admin via the first-account bootstrap rule (per the existing `resolve_or_create` behaviour where the very first account in the system gets `is_server_admin = TRUE`), it SHALL emit `ServerAdminGranted { account_id, source: ServerAdminGrantSource::FirstAccountBootstrap }` with `actor_account_id = None` and `acting_as = Some(...)`.
5. If the callback is in add-character mode (the `/auth/characters/add` flow with an authenticated session), and a new `eve_character` row is created for the existing account, it SHALL emit `CharacterAdded { account_id, eve_character_id, character_name }` with `actor_account_id = Some(account_id)` and `acting_as = None`. (In this flow the account exists, has a main, and the session predates the SSO redirect.)
6. If add-character mode claims an existing orphan rather than creating a fresh row, it SHALL emit `OrphanCharacterClaimed { account_id, eve_character_id, character_name }` with `actor_account_id = Some(account_id)` and `acting_as = None`.

The above emissions SHALL be ordered after `promote_if_no_main` so that any state needed for snapshot resolution is in place.

If any audit emission fails, the entire transaction (including the state change) SHALL be rolled back. This is the inherent behaviour of `record_in_tx` participating in the caller's transaction; the SSO callback service does not catch and ignore audit errors.

#### Scenario: First-character registration emits account_registered with non-null actor character
- **WHEN** a brand-new EVE character completes SSO and a new account is created
- **THEN** an `audit_log` row exists with `event_type = "account_registered"`, `actor_account_id = NULL`, `actor_character_id = <the signing-in EVE character ID>`, `actor_character_name = <the signing-in character name>`, and `details` containing `account_id`, `eve_character_id`, and `character_name`

#### Scenario: First account ever also emits server_admin_granted with bootstrap source
- **WHEN** the very first account is created via SSO (no prior `account` rows exist)
- **THEN** two `audit_log` rows exist for that transaction: `account_registered` and `server_admin_granted` with `details.source = "first_account_bootstrap"`

#### Scenario: Orphan-claim during login emits orphan_character_claimed
- **GIVEN** an `eve_character` row exists with `account_id IS NULL`
- **WHEN** that character completes SSO for the first time
- **THEN** an `audit_log` row exists with `event_type = "orphan_character_claimed"`, `actor_account_id = NULL`, `actor_character_id = <the EVE character ID>`, `actor_character_name = <the character name>`, and `details` containing `account_id`, `eve_character_id`, `character_name`

#### Scenario: Re-login of a soft-deleted account emits account_reactivated
- **GIVEN** an account with `status = 'soft_deleted'`
- **WHEN** one of its characters completes SSO and the account is reactivated
- **THEN** an `audit_log` row exists with `event_type = "account_reactivated"`, `actor_account_id = NULL`, `actor_character_id = <the logging-in character's EVE ID>`, `actor_character_name = <that character's name>`, and `details.account_id` matches the reactivated account

#### Scenario: Add-character flow emits character_added with the account's main as actor character
- **GIVEN** an authenticated session for an account whose main character is "Main Pilot"
- **WHEN** that account adds a second character via `/auth/characters/add` and SSO completes
- **THEN** an `audit_log` row exists with `event_type = "character_added"`, `actor_account_id = <the account ID>`, `actor_character_id = <Main Pilot's EVE ID>`, `actor_character_name = "Main Pilot"`, and `details` containing `eve_character_id` and `character_name` of the newly added character (not the main)

#### Scenario: Add-character flow claiming an orphan emits orphan_character_claimed with main actor
- **GIVEN** an authenticated session and an existing orphan `eve_character` row
- **WHEN** the account adds that orphan as a character via `/auth/characters/add` and SSO completes
- **THEN** an `audit_log` row exists with `event_type = "orphan_character_claimed"`, `actor_account_id = <the account ID>`, `actor_character_id = <the account's main EVE ID>`, `actor_character_name = <the main's name>`

#### Scenario: Audit emission failure rolls back the SSO callback transaction
- **GIVEN** a transient database failure that occurs during the audit emission step of the SSO callback transaction
- **WHEN** the transaction attempts to commit
- **THEN** the transaction is rolled back; no `eve_character` row is created or modified, no session is established, no audit row is written; the user-facing response is HTTP 5xx
