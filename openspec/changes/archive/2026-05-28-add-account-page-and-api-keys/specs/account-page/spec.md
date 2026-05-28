## ADDED Requirements

This capability defines the `/account` page in the SvelteKit frontend: a tabbed page that hosts API key management and account-lifecycle controls (currently just "Delete account"). It also defines the user-chip menu entry that links to it, and the migration of the existing "Delete account" affordance from `/characters`. Backend HTTP contracts for `/api/v1/keys` and `DELETE /api/v1/account` are owned by the `api-authentication` and `account-management` capabilities respectively and are unchanged by this capability.

### Requirement: /account route requires authentication

The frontend SHALL serve a route at `/account` (`frontend/src/routes/account/+page.{server.ts,svelte}`). The route SHALL require an authenticated session — the standard `+layout.server.ts` redirect-on-401 behaviour applies; `/account` SHALL NOT appear in the public-route list. The server load function SHALL fetch the caller's API keys via `GET /api/v1/keys` so the page renders without an additional client round-trip.

#### Scenario: Unauthenticated visitor is redirected
- **WHEN** a visitor with no session cookie navigates to `/account`
- **THEN** the standard layout-level redirect to `/login` SHALL apply (the route is not in the public-route list)

#### Scenario: Authenticated visitor sees their keys
- **WHEN** an authenticated visitor navigates to `/account`
- **THEN** the page SHALL render with the API keys tab active and the list of the caller's keys (sourced from `GET /api/v1/keys`) visible

### Requirement: /account is linked from the user-menu dropdown

The user-menu dropdown (`UserMenu.svelte`) SHALL include an `account` link that navigates to `/account`. The link SHALL be a real, enabled `<a href="/account">` — NOT an `aria-disabled` placeholder. The link SHALL replace the previously-disabled "Settings" placeholder at the same position in the menu (between `preferences` and `about`).

The link's visible text SHALL be the Paraglide message `user_menu_account`. The previous Paraglide message key `user_menu_settings` SHALL be removed; its English value SHALL change from `"settings"` to `"account"` and its German value from `"Konfiguration"` to `"Konto"`.

#### Scenario: account link present in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** the menu contains an enabled `<a href="/account">` rendering the `user_menu_account` message; no element renders `user_menu_settings` (the key no longer exists)

#### Scenario: account link is fully enabled
- **WHEN** the user-menu dropdown is open
- **THEN** the account link is NOT `aria-disabled`; it has a real `href` attribute and is keyboard-focusable

### Requirement: /account organises content into tabs

The `/account` page SHALL organise its content into tabs. Two tabs SHALL be present in this version of the capability: **API keys** (containing key list, create, and revoke) and **Danger zone** (containing the Delete account action). The tab interaction model SHALL match `/preferences`: tab state is component-local `$state` (NOT URL-backed), and switching tabs SHALL NOT submit, discard, or otherwise mutate any in-progress form on either tab.

#### Scenario: Tabs default to API keys
- **WHEN** the page first renders
- **THEN** the API keys tab is active and its content is visible; the Danger zone tab is inactive and its content is hidden

#### Scenario: Tab switch preserves in-progress create form
- **WHEN** a user opens the create-key form, types a name, then switches to the Danger zone tab and back
- **THEN** the create-key form SHALL still be open with the typed name preserved

### Requirement: API keys tab lists the caller's keys

The API keys tab SHALL render a list of the caller's keys, sourced from `GET /api/v1/keys`. Each row SHALL display the key's name, created timestamp, expiry (either an absolute date or the literal "Never" when `expires_at` is null), and a Revoke action. The list SHALL NOT display any plaintext key value (it is unavailable from `GET /api/v1/keys` by spec; this requirement makes the UI invariant explicit).

Rows whose `expires_at` is in the past relative to the client's clock SHALL be rendered de-emphasised (reduced foreground contrast) and SHALL display an "Expired" badge in the expiry column. Expired rows SHALL retain a usable Revoke action so the user can clean them up.

#### Scenario: Active keys are listed
- **WHEN** the caller has API keys with `expires_at` in the future or null
- **THEN** each key renders a row with name, created date, expiry (date or "Never"), and a Revoke button at normal contrast

#### Scenario: Expired keys are listed with a badge
- **WHEN** the caller has an API key with `expires_at` in the past
- **THEN** the row renders de-emphasised with an "Expired" badge in the expiry column and a usable Revoke button

#### Scenario: No plaintext is ever displayed in the list
- **WHEN** the list renders any number of rows
- **THEN** no row contains a value matching `erb_<43 chars>` (the plaintext format defined by `api-authentication`)

### Requirement: API keys tab renders an empty state when the caller has no keys

When the caller has zero API keys, the API keys tab SHALL render an explanatory empty state in place of the list. The empty state SHALL include short copy describing what API keys are and a primary call-to-action button that opens the create-key form (the same form invoked by `[+ New key]` in the populated view).

#### Scenario: Empty state renders when there are no keys
- **WHEN** the page renders for an account with zero API keys
- **THEN** the API keys tab shows explanatory copy and a button that, when clicked, reveals the create-key form

### Requirement: Create key form collects a name and expiry preset

The API keys tab SHALL provide a "New key" affordance that reveals an inline create form. The form SHALL collect:

- A **name** (non-empty string).
- An **expiry**, selected from presets `Never` / `In 30 days` / `In 90 days` / `In 1 year` / `Custom...`. Selecting `Custom...` SHALL reveal a date input. The frontend SHALL compute an absolute RFC3339 timestamp from the selected preset before submitting (end-of-day UTC for the custom date), or submit a null expiry for `Never`.

The form SHALL submit to a SvelteKit form action that calls the backend `POST /api/v1/keys`. The form SHALL use `use:enhance` so submission is non-navigating and the result is delivered via the `form` prop.

#### Scenario: Preset is converted to an RFC3339 timestamp
- **WHEN** the user selects `In 30 days` and submits the form
- **THEN** the form action receives a `expires_at` field whose value is approximately 30 days in the future, formatted as RFC3339, and forwards it to `POST /api/v1/keys`

#### Scenario: Never expiry submits as null
- **WHEN** the user selects `Never` and submits the form
- **THEN** the request body sent to `POST /api/v1/keys` has `expires_at: null`

#### Scenario: Empty name is rejected client-side
- **WHEN** the user submits the form with an empty name
- **THEN** the form does NOT call the backend; an inline validation message is shown

### Requirement: Newly-created plaintext is shown exactly once via a confirmation gate

On successful key creation, the page SHALL render an inline **reveal panel** containing the plaintext `key` field from the create response. The panel SHALL include:

- A prominent warning that this is the only time the key will be shown.
- The plaintext key rendered as selectable text.
- A `[Copy]` button that copies the plaintext to the clipboard via `navigator.clipboard.writeText`. If clipboard access fails, the panel SHALL display an inline message advising the user to select the key manually; the gate SHALL NOT consider this a failure.
- A checkbox labelled "I've saved this key somewhere safe" (or its i18n equivalent), unchecked by default.
- A `[Done]` button that is `disabled` while the checkbox is unchecked. Activating `[Done]` SHALL clear the reveal panel.

The plaintext SHALL be held in component-local state, NOT persisted to `sessionStorage`, `localStorage`, the URL, or any server-side store. Closing the reveal panel SHALL discard the plaintext; it SHALL NOT be recoverable thereafter. After the panel is dismissed, the keys list SHALL show the newly-created key's metadata row.

#### Scenario: Reveal panel renders the plaintext after creation
- **WHEN** the create form is submitted and the backend returns `201` with a plaintext `key`
- **THEN** the reveal panel renders with the warning copy, the plaintext key, a Copy button, the checkbox unchecked, and the Done button disabled

#### Scenario: Done button enables when the checkbox is ticked
- **WHEN** the user ticks the "I've saved this key somewhere safe" checkbox
- **THEN** the Done button becomes enabled

#### Scenario: Dismissing the panel discards the plaintext
- **WHEN** the user activates the Done button
- **THEN** the reveal panel is unmounted, the plaintext is no longer present in the DOM, and the list shows the new key's metadata row

#### Scenario: Plaintext is not persisted across page reload
- **WHEN** the user reloads the page while the reveal panel is open
- **THEN** the panel does NOT reappear and the plaintext is irrecoverable (it was only ever in component-local state)

#### Scenario: Clipboard failure does not block the gate
- **WHEN** the Copy button is clicked and `navigator.clipboard.writeText` rejects
- **THEN** the panel displays an inline message advising manual selection; the checkbox and Done button continue to function normally

### Requirement: Revoke uses the shared confirmation dialog

Revoking a key SHALL be gated by the shared `ConfirmDialog` component per the `frontend-patterns` capability. Clicking a row's Revoke button SHALL open the dialog (with title, body, and confirm-label copy describing the revoke action and naming the key); the dialog's confirm action SHALL trigger submission of a per-row SvelteKit form whose action calls `DELETE /api/v1/keys/:id`. Inline confirmations, `window.confirm()`, and full-page confirmation routes SHALL NOT be used.

On successful revocation the list SHALL refresh (the loader reruns) and the revoked row SHALL disappear. On failure the page SHALL display an inline error row beneath the offending row (matching the `/characters` `formError?.characterId === character.id` pattern), keyed by `keyId`.

#### Scenario: Revoke opens the confirmation dialog
- **WHEN** the user clicks Revoke on a row
- **THEN** the row's form is NOT submitted immediately; `ConfirmDialog` opens with the revoke copy

#### Scenario: Confirming the dialog revokes the key
- **WHEN** the user activates the destructive button in the open dialog
- **THEN** the row's form is submitted, the backend returns `204`, the loader reruns, and the row disappears from the list

#### Scenario: Cancelling the dialog leaves the key intact
- **WHEN** the user activates the cancel button in the open dialog
- **THEN** the dialog closes, the row's form is NOT submitted, and the row remains in the list

#### Scenario: Revoke failure renders an inline error keyed to the row
- **WHEN** the revoke form action returns an error result with `keyId` populated
- **THEN** the page renders an inline error row beneath the matching list row; other rows are unaffected

### Requirement: Danger zone tab hosts the Delete account action

The Danger zone tab SHALL host a single action — Delete account — that calls `DELETE /api/v1/account` via a SvelteKit form action. The action SHALL be gated by `ConfirmDialog` per the `frontend-patterns` capability. The visible copy (button label, dialog title, body, confirm label) SHALL be equivalent in meaning to the copy previously presented on `/characters`; the underlying soft-delete semantics defined by `account-management` are unchanged. On successful soft-delete the session cookie is cleared by the backend and the user SHALL be returned to `/login` (the existing post-delete navigation behaviour from `/characters` carries over).

The Delete account action SHALL NO LONGER be present on `/characters`. The `?/deleteAccount` form action and its associated UI elements (trigger button, `ConfirmDialog`, danger-zone heading and divider) SHALL be removed from `/characters`.

#### Scenario: Delete account button is on /account, not /characters
- **WHEN** a user opens `/account` and switches to the Danger zone tab
- **THEN** a Delete account button is visible; AND when the same user navigates to `/characters`, no Delete account button, danger-zone heading, or `?/deleteAccount` form is present

#### Scenario: Delete account is gated by ConfirmDialog
- **WHEN** the user clicks Delete account on the Danger zone tab
- **THEN** the form is NOT submitted immediately; `ConfirmDialog` opens with the delete-account copy; only on confirm is the form submitted

#### Scenario: Successful delete returns the user to /login
- **WHEN** the user confirms delete and the backend returns `204`
- **THEN** the session cookie is cleared (by the backend) and the user is navigated to `/login`

### Requirement: i18n keys for the moved Delete account action are renamed

The Paraglide message keys previously used for the Delete account UI on `/characters` SHALL be renamed to reflect their new home. The visible English and German values SHALL NOT change.

The renames SHALL be:

- `characters_delete_account` → `account_delete_account`
- `characters_delete_account_title` → `account_delete_account_title`
- `characters_delete_account_body` → `account_delete_account_body`
- `characters_delete_account_confirm` → `account_delete_account_confirm`
- `characters_danger_zone` → `account_danger_zone`

The old keys SHALL be removed from both `frontend/messages/en.json` and `frontend/messages/de.json`. Any code references SHALL be updated to use the new `m.account_*()` message functions.

#### Scenario: Old keys no longer exist
- **WHEN** the source tree is grepped for `characters_delete_account` or `characters_danger_zone`
- **THEN** no occurrences are found in source code, message catalogues, or compiled output

#### Scenario: New keys render the same visible copy
- **WHEN** the Danger zone tab and its confirmation dialog render
- **THEN** the visible English and German strings are byte-identical to those previously rendered under the `characters_*` keys
