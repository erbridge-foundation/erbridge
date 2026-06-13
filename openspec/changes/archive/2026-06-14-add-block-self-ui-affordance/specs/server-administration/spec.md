## ADDED Requirements

### Requirement: The block picker marks the admin's own characters as non-selectable

The admin block picker (`/admin/blocks`) SHALL visibly mark any search result that belongs to the currently authenticated admin as non-selectable, presenting a non-actionable indicator in place of the "Select" control. This mirrors the existing already-blocked indicator and surfaces the `cannot_block_self` rule before submission rather than only as a `409` after the fact. It is a presentation-layer affordance only; the backend `409 cannot_block_self` guard on `POST /api/v1/admin/blocks` remains the enforcement boundary and is unchanged.

A result SHALL be treated as belonging to the current admin when either:
- it is a local result whose `account_id` equals the current account's id (catching every character on the admin's own account, including alts), or
- it is an ESI result (which carries no `account_id`) whose `eve_character_id` matches one of the current account's characters.

#### Scenario: A local result on the admin's own account is non-selectable
- **WHEN** a local search returns a character whose `account_id` equals the authenticated admin's account id
- **THEN** the picker renders a non-actionable "you" indicator for that result and renders no "Select" control for it

#### Scenario: An ESI result matching one of the admin's own characters is non-selectable
- **WHEN** an ESI search returns a character whose `eve_character_id` matches one of the authenticated admin's own characters
- **THEN** the picker renders a non-actionable "you" indicator for that result and renders no "Select" control for it

#### Scenario: A result on another account remains selectable
- **WHEN** a search returns a character that belongs to neither the admin's account nor the block list
- **THEN** the picker renders the normal "Select" control for that result

### Requirement: The revoke confirmation warns when an admin revokes their own rights

The admin revoke flow (`/admin/admins`) SHALL display a prominent warning in the revoke confirmation dialog when the account being revoked is the currently authenticated admin's own account. Self-revoke remains permitted — the backend deliberately allows an admin to revoke their own rights as long as the last-admin guard holds — so this is a footgun warning, not a block: the confirm control still revokes.

#### Scenario: Revoking your own account shows a warning
- **WHEN** an admin opens the revoke confirmation for the account that equals their own account id
- **THEN** the dialog shows a warning that they are about to lose their own admin access
- **AND** the confirm control still performs the revoke (the action is not prevented)

#### Scenario: Revoking another account shows no self-warning
- **WHEN** an admin opens the revoke confirmation for an account that is not their own
- **THEN** the dialog shows no self-revoke warning
