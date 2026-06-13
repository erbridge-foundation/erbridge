## ADDED Requirements

### Requirement: Admin can hard-delete an account with a deletion preview

The admin surface SHALL expose a hard-delete action for an account, gated by the `AdminAccount` extractor, that performs the irreversible `DELETE FROM account` defined by the `account-hard-delete` capability. This is distinct from the user-facing soft-delete: it removes the account row outright (cascading its characters, sessions, and API keys) rather than flipping a status.

The action SHALL be preceded by a server-provided deletion preview reporting the blast radius: counts of characters, sessions, and API keys that will be **removed**, and counts of maps and ACLs that will become **unowned** (owner reference nulled, rows retained). The preview SHALL convey that audit history is preserved. The admin UI SHALL render the preview alongside an explicit, irreversible-action confirmation before dispatching the delete. The last-server-admin guard SHALL apply (HTTP 409 `cannot_remove_last_server_admin`).

This action is the operator's resolution for an unreachable duplicate account — notably the account a buyer accidentally creates by logging in fresh with a transferred character (see the `eve-sso-auth` known limitation): the admin hard-deletes the spare account, after which the human adds the character to their original account normally.

#### Scenario: Admin sees a preview before hard-deleting
- **WHEN** an admin selects an account for hard-deletion
- **THEN** the UI shows a preview of how many characters/sessions/API keys will be removed and how many maps/ACLs will become unowned, plus an explicit "this cannot be undone" confirmation

#### Scenario: Admin confirms and the account is hard-deleted
- **WHEN** an admin confirms the hard-delete
- **THEN** the backend deletes the `account` row (cascading characters/sessions/keys, nulling map/ACL/audit/block owner references), emits an audit event, and the account no longer appears in the account list

#### Scenario: Hard-delete respects the last-admin guard
- **WHEN** an admin attempts to hard-delete an account that would leave no other active server admin
- **THEN** the request is refused with HTTP 409 `cannot_remove_last_server_admin` and nothing is deleted

#### Scenario: Hard-delete action is admin-only
- **WHEN** a non-admin attempts to reach the hard-delete action or endpoint
- **THEN** it is rejected fail-closed by the `AdminAccount` extractor
