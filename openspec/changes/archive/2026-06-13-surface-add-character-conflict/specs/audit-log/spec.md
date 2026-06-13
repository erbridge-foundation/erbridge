# audit-log — delta for surface-add-character-conflict

## ADDED Requirements

### Requirement: character_add_rejected_bound_elsewhere event

The `AuditEvent` catalogue SHALL gain a `CharacterAddRejectedBoundElsewhere { account_id, eve_character_id }` variant with `event_type = "character_add_rejected_bound_elsewhere"`. Like `BlockedLoginRejected`, it records a rejected *attempt* rather than a committed state change. It SHALL be emitted by the SSO callback's add-character path when the presented character is already bound to a different account, with:

- `actor_account_id` = the session account that attempted the add (an authenticated session exists, unlike `BlockedLoginRejected`);
- `target()` → target_type `"character"`, target_id the `eve_character_id`, name not carried (NULL);
- `details()` containing `eve_character_id` (the owning account is deliberately NOT recorded in details — an audit reader with DB access can resolve it, but the event must not casually leak account linkage into the admin audit browser).

#### Scenario: Rejected add is recorded with the session actor
- **WHEN** the add-character flow is refused because the character is bound to another account
- **THEN** an audit row exists with `event_type = "character_add_rejected_bound_elsewhere"`, `actor_account_id` = the session account, `target_type = "character"`, and `target_id` = the character's EVE id

#### Scenario: The owning account is not leaked in details
- **WHEN** the `character_add_rejected_bound_elsewhere` row is inspected via the admin audit browser
- **THEN** `details` contains the `eve_character_id` but not the other account's id
