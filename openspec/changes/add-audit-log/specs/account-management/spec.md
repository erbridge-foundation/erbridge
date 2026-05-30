## ADDED Requirements

### Requirement: Account-management mutating endpoints emit audit events

The following endpoints SHALL emit audit events (per the `audit-log` capability) into the same transaction that performs the state change. Each emission SHALL use `actor_account_id = Some(<authenticated account>)` and `acting_as = None`, since all of these endpoints require an authenticated session or bearer key.

- `DELETE /api/v1/account` SHALL emit `AccountDeletionRequested { account_id }`. The emission SHALL occur inside the same transaction as the `account.status = 'soft_deleted'` transition and the character-token clearing required by the existing soft-delete requirement.
- `POST /api/v1/characters/:id/set-main` SHALL emit `CharacterSetMain { account_id, eve_character_id }` where `eve_character_id` is the EVE ID of the newly-promoted character. The emission SHALL occur inside the same transaction as the `is_main` flip.
- `DELETE /api/v1/characters/:id` SHALL emit `CharacterRemoved { account_id, eve_character_id }` where `eve_character_id` is the EVE ID of the character being removed. The emission SHALL occur inside the same transaction as the `DELETE FROM eve_character` statement.

If any audit emission fails, the entire transaction (including the state change) SHALL be rolled back. The endpoint SHALL NOT swallow audit errors.

For `CharacterSetMain`, the actor-character snapshot reflects the account's main *at the time of the emission*. Because the audit row is written inside the same transaction as (and conventionally *before*) the `is_main` flip commits, the snapshot SHALL be the *outgoing* main, not the incoming one. This preserves the property that the main-history of an account is reconstructible from the audit log: the `details.eve_character_id` of each `character_set_main` row identifies the *new* main, while the snapshot identifies the main that was being displaced.

#### Scenario: DELETE /api/v1/account writes an account_deletion_requested audit row
- **WHEN** an authenticated caller successfully soft-deletes their account via `DELETE /api/v1/account`
- **THEN** an `audit_log` row exists with `event_type = "account_deletion_requested"`, `actor_account_id = <the caller's account ID>`, `actor_character_id` / `actor_character_name` populated from that account's main, and `details = {}` (empty object — actor carries the account)

#### Scenario: POST /api/v1/characters/:id/set-main writes a character_set_main audit row
- **GIVEN** an account whose current main is character A, and a non-main character B owned by the same account
- **WHEN** the caller `POST /api/v1/characters/<B.id>/set-main`
- **THEN** an `audit_log` row exists with `event_type = "character_set_main"`, `actor_account_id = <the caller>`, `actor_character_id = A.eve_character_id` (the outgoing main, snapshotted before the flip), `actor_character_name = A.name`, and `details.eve_character_id = B.eve_character_id` (the incoming main)

#### Scenario: DELETE /api/v1/characters/:id writes a character_removed audit row
- **WHEN** an account removes one of its characters via `DELETE /api/v1/characters/:id` and the request succeeds (HTTP 204)
- **THEN** an `audit_log` row exists with `event_type = "character_removed"`, `actor_account_id = <the caller>`, and `details.eve_character_id` equal to the removed character's EVE ID

#### Scenario: Audit emission failure rolls back the state change
- **GIVEN** a transient database failure on the audit emission within `DELETE /api/v1/account`
- **WHEN** the transaction attempts to commit
- **THEN** the transaction is rolled back; `account.status` remains `'active'`; no `audit_log` row is written; the client sees an HTTP 5xx response

#### Scenario: Rejected requests do not write audit rows
- **WHEN** `DELETE /api/v1/characters/:id` is rejected with HTTP 409 (e.g. `cannot_remove_main` or `cannot_remove_last_character`)
- **THEN** no `audit_log` row is written for the rejected request
