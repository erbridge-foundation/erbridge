## MODIFIED Requirements

### Requirement: Admin character search and token-state visibility

The admin UI SHALL provide a Characters tab that renders, as a datagrid, every account known to the server — one row per account — so a server admin can see and triage the roster and its token problems without first issuing a search. The grid SHALL read from the already-loaded admin accounts list (`GET /api/v1/admin/accounts`) and SHALL NOT perform a character-name search or any outbound ESI lookup; arbitrary/orphan character lookup is out of scope for this surface.

Each account row SHALL be labelled by the account's main character's name (the character flagged `is_main`); if no character is flagged main, the row SHALL fall back to the first character by name. Each row SHALL surface a roll-up of the account's worst token state (counts of characters whose `token_status` is `token_expired` and `owner_mismatch`) so that token problems are visible without further interaction. A row SHALL expand to reveal every character on that account with its `token_status`.

The grid SHALL support a free-text filter that matches both the account's main name and its alt names (so filtering by an alt name surfaces that alt's account row), account-level status filtering that surfaces accounts having any character whose `token_status` is `token_expired` and/or `owner_mismatch`, and sortable columns.

#### Scenario: Admin sees the account roster without searching

- **WHEN** a server admin opens the Characters tab
- **THEN** the grid lists every account as a row labelled by its main character (or first character by name if none is main), with no search step required

#### Scenario: Admin expands an account to inspect its characters

- **WHEN** a server admin expands an account row
- **THEN** every character on that account is shown with its `token_status`

#### Scenario: Admin surfaces accounts with token problems

- **WHEN** a server admin filters or sorts the grid by token state
- **THEN** accounts having any character whose `token_status` is `token_expired` (and `owner_mismatch`) are surfaced together, and each such account's row shows its problem roll-up without being expanded

#### Scenario: Admin filters by character name

- **WHEN** a server admin types a name fragment into the grid's text filter
- **THEN** rows whose main name or any alt name matches the fragment are shown
