## ADDED Requirements

### Requirement: Server admin can hard-delete an account

The system SHALL expose a server-admin-only endpoint that irreversibly deletes an `account` row via `DELETE FROM account` (a true row deletion, distinct from the user-facing soft-delete). The endpoint SHALL be gated by the `AdminAccount` extractor (cookie-authenticated server admins only) and SHALL be the in-app resolution for an unreachable duplicate account (see the `eve-sso-auth` known limitation for a buyer who logs in fresh with a transferred character).

The deletion SHALL rely on the existing foreign-key behaviour, which is already designed for this:

- `eve_character.account_id`, `session.account_id`, `api_key.account_id` are `ON DELETE CASCADE` — these rows are removed with the account.
- `map.owner_account_id`, `acl.owner_account_id`, `audit_log.actor_account_id`, `blocked_eve_character.blocked_by` are `ON DELETE SET NULL` — these rows SURVIVE, losing only their back-reference to the deleted account. Maps and ACLs become unowned (NOT deleted). Audit history is preserved: each `audit_log` row retains its snapshot `actor_character_id` / `actor_character_name` and self-contained JSONB `details`, so no audit information is lost.

The endpoint SHALL evaluate the last-server-admin guard inside the same transaction as the delete: if deleting the target would leave no other active server admin, the request SHALL be refused with HTTP 409 `cannot_remove_last_server_admin`. The deletion SHALL emit an audit event recording the deleted account's id and its `last_known_main_character_name` snapshot so the event remains meaningful after the row is gone.

#### Scenario: Admin hard-deletes an account and its private rows cascade away
- **WHEN** a server admin hard-deletes an account that owns characters, sessions, and API keys
- **THEN** the `account` row and all its `eve_character`, `session`, and `api_key` rows are removed, and the response is success

#### Scenario: Co-owned resources survive as unowned
- **WHEN** the deleted account owned maps and ACLs and was the actor on audit rows
- **THEN** those `map` / `acl` rows persist with `owner_account_id = NULL`, and those `audit_log` rows persist with `actor_account_id = NULL` while retaining their snapshot actor character id/name and JSONB details

#### Scenario: Hard-delete is gated to admins
- **WHEN** a non-admin (or unauthenticated) caller invokes the hard-delete endpoint
- **THEN** the request is rejected by the `AdminAccount` extractor (fail-closed) and no deletion occurs

#### Scenario: Last-admin guard blocks deleting the final admin
- **WHEN** a server admin attempts to hard-delete an account such that no other active server admin would remain
- **THEN** the request is refused with HTTP 409 `cannot_remove_last_server_admin` and the account is not deleted

#### Scenario: Hard-delete is audited
- **WHEN** an account is hard-deleted
- **THEN** an audit event is recorded carrying the deleted account's id and its `last_known_main_character_name` snapshot, with the acting admin as actor

### Requirement: Hard-delete is preceded by a deletion preview

Before performing a hard-delete the system SHALL make available a blast-radius preview of what the deletion will affect, so an admin can make an informed, irreversible decision. The preview SHALL report counts of rows that will be **removed** (characters, sessions, API keys) and rows that will become **unowned but are NOT deleted** (owned maps, owned ACLs). The preview SHALL convey that audit history is preserved and SHALL NOT describe it as lost.

The admin UI SHALL present this preview together with an explicit confirmation step that communicates the action is permanent and cannot be undone before the deletion is dispatched.

#### Scenario: Preview reports removed and unowned counts
- **WHEN** an admin requests the preview for an account before deleting it
- **THEN** the preview reports the number of characters, sessions, and API keys that will be removed, and the number of maps and ACLs that will become unowned

#### Scenario: Preview does not report audit loss
- **WHEN** the deletion preview is rendered
- **THEN** it communicates that audit history is preserved and does not claim audit entries will be deleted or lost

#### Scenario: Confirmation precedes an irreversible delete
- **WHEN** an admin initiates a hard-delete from the admin UI
- **THEN** the preview and an explicit "this cannot be undone" confirmation are shown before the deletion request is sent
