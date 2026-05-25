## MODIFIED Requirements

### Requirement: /about is linked from the user-menu dropdown

The user-menu dropdown (`UserMenu.svelte`) SHALL include an `about` link that navigates to `/about`. The link SHALL be positioned **below** the `preferences` and `settings` items and **above** the divider, so the menu order is `preferences`, `settings`, `about`, divider, `log out`. The `about` link SHALL be a real, enabled `<a href="/about">` — not a greyed-out placeholder. Selecting it SHALL close the dropdown.

(Re-ordered by the `accessibility-preferences` change: once `preferences` became a real route it took the top slot, and `about` moved down to sit just above `log out`. This supersedes the original "positioned above the preferences and settings placeholders / first item is about" ordering.)

The user-menu is only visible to authenticated users (it is part of the GlobalNav user chip). Unauthenticated users reach `/about` by direct URL or external link.

#### Scenario: about link present in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** an `about` link with `href="/about"` is present, fully enabled (not `aria-disabled`), positioned below `preferences`/`settings` and above the `log out` divider
