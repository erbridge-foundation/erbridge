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
