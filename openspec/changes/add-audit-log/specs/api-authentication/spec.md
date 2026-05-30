## ADDED Requirements

### Requirement: API key management endpoints emit audit events

The following endpoints SHALL emit audit events (per the `audit-log` capability) into the same transaction that performs the state change. Each emission SHALL use `actor_account_id = Some(<authenticated account>)` and `acting_as = None`, since both endpoints require authentication.

- `POST /api/v1/keys` SHALL emit `ApiKeyCreated { account_id, key_id, name }` where `key_id` is the UUID of the newly-inserted `api_key` row and `name` is the user-supplied key name. The emission SHALL occur inside the same transaction as the `INSERT INTO api_key` statement.
- `DELETE /api/v1/keys/:id` SHALL emit `ApiKeyRevoked { account_id, key_id }` where `key_id` is the UUID of the deleted `api_key` row. The emission SHALL occur inside the same transaction as the `DELETE FROM api_key` statement.

If any audit emission fails, the entire transaction (including the state change) SHALL be rolled back. The endpoint SHALL NOT swallow audit errors.

Rejected requests (HTTP 400 for missing name, HTTP 409 for duplicate name, HTTP 404 for non-owner revoke or unknown key) SHALL NOT write audit rows. Audit rows correspond to *committed state changes*, not to rejected requests.

#### Scenario: POST /api/v1/keys writes an api_key_created audit row
- **WHEN** an authenticated caller successfully creates a key via `POST /api/v1/keys` with `name = "ci"`
- **THEN** an `audit_log` row exists with `event_type = "api_key_created"`, `actor_account_id = <the caller>`, `actor_character_id` / `actor_character_name` populated from that account's main, and `details` containing `key_id` (the new row's UUID) and `name = "ci"`

#### Scenario: DELETE /api/v1/keys/:id writes an api_key_revoked audit row
- **WHEN** an authenticated owner revokes one of their keys via `DELETE /api/v1/keys/:id` and the request succeeds (HTTP 204)
- **THEN** an `audit_log` row exists with `event_type = "api_key_revoked"`, `actor_account_id = <the caller>`, and `details.key_id` equal to the deleted row's UUID

#### Scenario: Rejected create does not write audit row
- **WHEN** `POST /api/v1/keys` is rejected with HTTP 400 (missing name) or HTTP 409 (duplicate name)
- **THEN** no `audit_log` row is written

#### Scenario: Rejected revoke does not write audit row
- **WHEN** `DELETE /api/v1/keys/:id` is rejected with HTTP 404 (key not found or owned by another account)
- **THEN** no `audit_log` row is written

#### Scenario: Audit emission failure rolls back the create
- **GIVEN** a transient database failure on the audit emission within `POST /api/v1/keys`
- **WHEN** the transaction attempts to commit
- **THEN** the transaction is rolled back; no `api_key` row is created; no `audit_log` row is written; the client sees an HTTP 5xx response
