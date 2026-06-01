## ADDED Requirements

### Requirement: Owner hash is captured on every callback

The system SHALL parse the `owner` claim from the EVE SSO access-token JWT during the OAuth2 callback and persist it as `eve_character.owner_hash`. The hash SHALL be written on every successful callback path — first link, orphan-claim, and re-auth — so the stored value always reflects the most recently presented claim for that character.

The `owner_hash` column SHALL be nullable. A null stored value means "not yet observed" and SHALL NOT be treated as a transfer; the current login records the hash for future comparison.

#### Scenario: First login records the owner hash

- **WHEN** a callback links a character that has no existing `eve_character` row
- **THEN** the inserted row's `owner_hash` is set to the `owner` claim from the access-token JWT

#### Scenario: Re-auth with an unchanged owner hash records the same hash

- **WHEN** a callback resolves an existing character whose stored `owner_hash` equals the presented `owner` claim
- **THEN** the row is upserted normally and `owner_hash` remains that value; no transfer enforcement runs

#### Scenario: A null stored owner hash is not a transfer

- **WHEN** a callback resolves an existing character whose stored `owner_hash IS NULL`
- **THEN** the presented `owner` claim is recorded and no transfer enforcement runs

### Requirement: Owner-hash change is detected as a character transfer

The system SHALL treat an existing character whose stored `owner_hash` is non-null and differs from the presented `owner` claim as a **transferred character**. Detection SHALL occur within the callback's persistence transaction, before the character's row is re-linked to the authenticating account.

#### Scenario: Changed owner hash triggers transfer enforcement

- **WHEN** a callback resolves an existing character with `account_id` set, a non-null stored `owner_hash`, and the presented `owner` claim differs from it
- **THEN** the system runs transfer enforcement (sever previous owner, detach, audit) in the same transaction before re-linking the character

### Requirement: A detected transfer severs the previous owner's access

On a detected transfer, within the same transaction and before the character is re-linked, the system SHALL, for the account that currently owns the character (the previous owner):

1. Clear the credential columns (`encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at`, `scopes`) on **all** of that account's `eve_character` rows.
2. Delete **all** of that account's `session` rows, so its session cookie no longer resolves and the next request forces a fresh login.

The system SHALL NOT clear sessions or credentials for the authenticating (new owner's) account; that account completes the login normally and receives a fresh session and fresh tokens for the transferred character.

#### Scenario: Previous owner's tokens are wiped across all their characters

- **WHEN** a transfer is detected for a character previously owned by account A
- **THEN** every `eve_character` row belonging to account A has `encrypted_access_token`, `encrypted_refresh_token`, `access_token_expires_at` set to NULL and `scopes` emptied

#### Scenario: Previous owner's sessions are cleared

- **WHEN** a transfer is detected for a character previously owned by account A
- **THEN** all `session` rows for account A are deleted, and account A's existing session cookie no longer resolves to a session

#### Scenario: New owner is not signed out

- **WHEN** a transfer is detected and the authenticating account B completes the callback
- **THEN** account B's sessions are not deleted and B receives a valid session cookie and fresh tokens for the transferred character

### Requirement: A transferred character is detached and re-linked to the new owner

On a detected transfer, after severing the previous owner, the system SHALL detach the character by setting its `account_id = NULL`, clearing `is_main`, and storing the new `owner_hash`. The callback SHALL then proceed through the existing orphan-claim path, which re-links the character to the authenticating account with fresh tokens.

#### Scenario: Transferred character is claimed by the new owner

- **WHEN** a transfer is detected for a character logging in as account B
- **THEN** the character's row is detached to `account_id = NULL` and then claimed by account B via the orphan-claim path, ending with `account_id = B`, fresh encrypted tokens, and the new `owner_hash`

#### Scenario: New owner's first character is promoted to main

- **WHEN** the transferred character is the first character linked to account B (B has no `is_main = TRUE` row after the claim)
- **THEN** the same transaction sets `is_main = TRUE` on the transferred character, consistent with the first-character-promotes-to-main rule

### Requirement: A detected transfer is recorded in the audit log

On a detected transfer, the system SHALL emit an audit event recording the transferred character's `eve_character_id`, the previous owner's account id, and the new owner's account id.

#### Scenario: Transfer emits an audit event

- **WHEN** a transfer is detected and enforced for a character moving from account A to account B
- **THEN** an audit event of the character-transfer kind is written with the character's `eve_character_id`, `old_account_id = A`, and `new_account_id = B`
