## ADDED Requirements

### Requirement: Account carries a denormalized last-known-main identity snapshot

Because an ERB account's human-readable identity is derived entirely from its characters, an account that loses all its characters would become unnameable and unfindable. To keep such an account identifiable, the `account` row SHALL carry a denormalized snapshot of its main character: `last_known_main_character_id` (BIGINT, nullable, holding the main's `eve_character_id`) and `last_known_main_character_name` (TEXT, nullable, holding the main's name).

These columns are a **display/identity snapshot, never a foreign key and never a join key**. `last_known_main_character_id` SHALL NOT reference `eve_character`; the snapshot must survive the referenced character row being detached or removed. The single source of truth for "which live character is main" remains the `is_main = TRUE` flag.

The snapshot SHALL be maintained in the same transaction as every change to which character is main:

- When `characters::set_main` promotes a character, it SHALL set the owning account's `last_known_main_*` to that character's `eve_character_id` and name.
- When `promote_if_no_main` promotes a character, it SHALL set the owning account's `last_known_main_*` to that character's `eve_character_id` and name. (Because `promote_if_no_main` returns only whether it promoted, the caller SHALL pass the promoted character's `eve_character_id` and name so the snapshot can be written.)

A null snapshot SHALL mean only "no main has ever been observed" (an account transiently before its first character).

#### Scenario: Setting a main updates the account snapshot
- **WHEN** a character is promoted to main via `set_main`
- **THEN** in the same transaction the owning account's `last_known_main_character_id` and `last_known_main_character_name` are set to that character's `eve_character_id` and name

#### Scenario: Auto-promotion updates the account snapshot
- **WHEN** `promote_if_no_main` promotes a character on an account that had no main
- **THEN** in the same transaction the account's `last_known_main_*` snapshot is set to that character's `eve_character_id` and name

#### Scenario: Snapshot survives the main character leaving the account
- **WHEN** the character recorded in `last_known_main_*` is detached or removed and no new main is promoted (the account is now empty)
- **THEN** the `last_known_main_character_id` and `last_known_main_character_name` columns retain the last-observed values, keeping the account nameable

### Requirement: An emptied account becomes orphaned, not deleted

The `account.status` domain SHALL include a terminal value `'orphaned'`, distinct from `'soft_deleted'`. `'soft_deleted'` means "owner-recoverable by logging back in"; `'orphaned'` means "unreachable — the account has zero characters, so no SSO login can ever resolve to it again." An orphaned account SHALL NOT be reactivated by the login self-heal path (which keys on resolving a character to the account, impossible with zero characters).

When the last character is detached from an account (e.g. by transfer detection in the `eve-sso-auth` capability), the same transaction SHALL set that account's `status = 'orphaned'`. The account row SHALL be retained, not deleted, so its `last_known_main_*` snapshot and any resources it owns remain visible to admins.

#### Scenario: Detaching the last character orphans the account
- **WHEN** an account's only character is detached
- **THEN** in the same transaction the account's `status` becomes `'orphaned'` and the row is retained with its `last_known_main_*` snapshot intact

#### Scenario: Orphaned status is distinct from soft-deleted
- **WHEN** an account is `'orphaned'`
- **THEN** its status is not `'soft_deleted'`, and the login self-heal/reactivation path does not restore it to `'active'` (no character can resolve to it)

#### Scenario: Orphaned account remains visible to admins
- **WHEN** an admin views accounts
- **THEN** an orphaned account is listed and is nameable via its `last_known_main_character_name`, and any maps or ACLs it owned remain visible
