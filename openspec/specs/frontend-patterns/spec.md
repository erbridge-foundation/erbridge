## Purpose

Shared SvelteKit-frontend primitives that span multiple capabilities. This capability is intentionally broad-named: the next shared frontend primitive (toasts, form-error rendering, loading states, etc.) can land here without spinning up a new capability. Current scope covers the destructive-action confirmation modal (a single shared component used by every state-mutating action whose effects are not symmetrically reversible from within the same view) and the stale-version reload banner (a single shared component that prompts the user to reload when a newer UI build has been deployed).

## Requirements

### Requirement: A shared confirmation modal gates every destructive frontend action

The frontend SHALL provide a single shared confirmation modal component (canonical path: `frontend/src/lib/components/ConfirmDialog.svelte`, per the `sveltekit-node` skill's component-layout rule). The component SHALL be the only confirmation pattern used by destructive actions in the SvelteKit frontend. Inline confirmation buttons (e.g. button text that flips to "confirm?"), native `window.confirm()`, and ad hoc full-page confirmation routes SHALL NOT be used for destructive actions.

The component SHALL accept the following props:

- `open: boolean` — controls visibility. The caller owns the open state.
- `tone: "danger"` — semantic colouring. Only `"danger"` is defined in this version of the spec; the prop exists so future non-destructive tones can be added without a breaking API change.
- `title: Snippet` — Svelte 5 snippet rendering the dialog title.
- `body: Snippet` — Svelte 5 snippet rendering the dialog body.
- `confirmLabel: Snippet` — Svelte 5 snippet rendering the destructive button's label.
- `onCancel: () => void` — invoked when the user cancels (clicks cancel, presses `Escape`, or clicks the backdrop).
- `onConfirm: () => void` — invoked when the user activates the destructive button.

The component SHALL NOT own form submission or any data mutation. Callers are responsible for whatever happens on confirm (e.g. calling `formEl.requestSubmit()` for `use:enhance` forms, `fetch(...)` for AJAX-driven actions, navigation for redirect-style actions). This keeps the primitive form-agnostic and reusable across capabilities whose destructive actions are not all form-based.

The component SHALL NOT be used for non-destructive dialogs (informational notices, multi-step forms, etc.). If a future surface needs a generic dialog primitive, it is a separate component — not a generalisation of this one — because this component's shape encodes the destructive-action copy contract defined below.

#### Scenario: A destructive form action is gated by the modal
- **GIVEN** a destructive action on the frontend (e.g. `?/remove` on `/characters`)
- **WHEN** the user clicks the destructive button
- **THEN** the form is NOT submitted immediately; the `ConfirmDialog` opens with the action's title, body, and confirm label

#### Scenario: Confirming submits the form
- **GIVEN** the `ConfirmDialog` is open for a destructive form action
- **WHEN** the user clicks the destructive button inside the dialog
- **THEN** the dialog's `onConfirm` callback fires and the underlying form is submitted (typically via `formEl.requestSubmit()`); the existing `use:enhance` behaviour and server-side action contract are unchanged

#### Scenario: Cancelling does not submit
- **GIVEN** the `ConfirmDialog` is open for a destructive form action
- **WHEN** the user clicks cancel, presses `Escape`, or clicks the backdrop
- **THEN** the dialog closes and no submission occurs

#### Scenario: Inline confirmation buttons are not used
- **WHEN** a frontend route or component implements a destructive action
- **THEN** the action does NOT use an inline two-step button pattern (e.g. text flipping from `remove` to `confirm remove?`), `window.confirm()`, or any pattern other than `ConfirmDialog`

### Requirement: Destructive-confirmation copy follows a structured contract

Every invocation of `ConfirmDialog` for a destructive action SHALL follow this copy structure:

- **Title**: `<destructive verb> <object>?` — e.g. `Delete map?`, `Remove character?`, `Delete account?`. The verb SHALL be the same verb used on the triggering button (no synonym swap between button and dialog title). The object SHALL be specific enough to identify what is being affected; where helpful (e.g. removing a specific named entity), the object SHALL be the actual name as rendered to the user.
- **Body**: one sentence, present tense, describing the consequence in user-visible terms. The body SHALL NOT use the phrases `Are you sure?`, `This will…`, or `Please confirm…`. It SHALL describe what happens, in the active or stative present, using the actual name where helpful.
- **Confirm label**: the destructive verb echoing the triggering button — e.g. `delete map`, `remove character`, `delete account`. The label SHALL NOT be a generic word (`confirm`, `yes`, `OK`, `proceed`); it SHALL repeat the verb so muscle-memory clickers must read the actual word.
- **Cancel label**: `cancel`. Always.

Examples that conform:

- Title `Delete account?` · body `Your account will be deactivated. To restore it, log back in within 30 days; after that, your data is permanently removed.` · confirm `delete account` · cancel `cancel`.
- Title `Remove Jita Trader?` · body `This character will be removed from your account. You can add them again at any time via add character and performing an EVE login.` · confirm `remove character` · cancel `cancel`.

#### Scenario: Generic confirm labels are rejected in review
- **WHEN** a frontend change proposes destructive-action copy with a generic confirm label (`confirm`, `yes`, `OK`, `proceed`)
- **THEN** the change is amended to use the destructive verb before merge

#### Scenario: Bodies that ask "Are you sure?" are rejected in review
- **WHEN** a frontend change proposes destructive-action copy whose body uses `Are you sure?`, `This will…`, or `Please confirm…`
- **THEN** the change is amended to describe the consequence directly before merge

### Requirement: Strict coverage policy with exception process

A frontend action SHALL go through `ConfirmDialog` when **all** of the following hold:

- (a) the action mutates server state, AND
- (b) the mutation is not reversible by a symmetric undo within the same view, where "symmetric undo" means a UI action that produces the inverse state with no data loss.

A frontend action SHALL NOT use `ConfirmDialog` when **any** of the following holds:

- (a) the action only navigates or redirects without mutating server state (e.g. `re-auth` which redirects to EVE SSO, `log out` which clears the local session);
- (b) the action has a built-in symmetric undo within the same view (e.g. "set main: A" can be undone by "set main: B");
- (c) the action is an idempotent no-op-on-failure (e.g. "mark read" where re-marking is harmless).

A future change that wants to use a different confirmation pattern for a specific destructive action (e.g. an inline two-step pattern for a high-frequency low-stakes case) SHALL propose the exception in its own change proposal, describing the action, the proposed pattern, and the trade-off. Exceptions SHALL NOT be granted by default; the strict policy stands until amended.

#### Scenario: A destructive action without symmetric undo uses the modal
- **WHEN** a frontend feature ships an action that mutates server state and has no symmetric undo
- **THEN** the action uses `ConfirmDialog` to gate the mutation

#### Scenario: A symmetric-undo action does not use the modal
- **WHEN** a frontend feature ships an action whose effect can be reversed by another in-view action with no data loss (e.g. "set main")
- **THEN** the action does NOT use `ConfirmDialog`

#### Scenario: A redirect-only action does not use the modal
- **WHEN** a frontend feature ships an action that only navigates or redirects (e.g. re-auth, log out)
- **THEN** the action does NOT use `ConfirmDialog`

#### Scenario: An exception is proposed in a change
- **WHEN** a change author believes a destructive action deserves a non-modal pattern
- **THEN** the change's proposal describes the action and the proposed pattern; the exception is reviewed alongside the rest of the change; the policy here is unchanged until a future change amends it

### Requirement: The modal meets WAI-ARIA alertdialog conventions

`ConfirmDialog` SHALL implement the WAI-ARIA `alertdialog` pattern:

- The dialog root SHALL have `role="alertdialog"`.
- The dialog root SHALL have `aria-modal="true"`.
- The dialog root SHALL be labelled by its title via `aria-labelledby` pointing to the title element's id, and SHALL be described by its body via `aria-describedby` pointing to the body element's id.
- When the dialog opens, focus SHALL move to the **cancel** button (not the destructive button). This protects against `Enter`-key reflex submission.
- While the dialog is open, focus SHALL be trapped within the dialog — `Tab` and `Shift+Tab` SHALL cycle only through the dialog's focusable elements (cancel button and destructive button).
- Pressing `Escape` SHALL invoke `onCancel`.
- Clicking the backdrop (the region outside the dialog) SHALL invoke `onCancel`. Clicking inside the dialog (outside the buttons) SHALL NOT close the dialog.
- When the dialog closes, focus SHALL return to the element that opened it (typically the triggering destructive button).
- The dialog SHALL NOT auto-focus the destructive button under any circumstance. There is no keyboard shortcut for confirm — a deliberate click or `Enter` on the focused destructive button (reached by `Tab`) is required.

#### Scenario: Dialog announces itself to assistive tech
- **WHEN** the dialog opens
- **THEN** the dialog root has `role="alertdialog"`, `aria-modal="true"`, an `aria-labelledby` pointing to the title element, and an `aria-describedby` pointing to the body element

#### Scenario: Cancel is the default-focused element
- **WHEN** the dialog opens
- **THEN** the cancel button receives keyboard focus (not the destructive button)

#### Scenario: Escape cancels
- **GIVEN** the dialog is open
- **WHEN** the user presses `Escape`
- **THEN** `onCancel` fires and the dialog closes

#### Scenario: Backdrop click cancels
- **GIVEN** the dialog is open
- **WHEN** the user clicks the backdrop outside the dialog body
- **THEN** `onCancel` fires and the dialog closes

#### Scenario: Focus is trapped within the dialog
- **GIVEN** the dialog is open with two focusable buttons (cancel, destructive)
- **WHEN** the user presses `Tab` from the destructive button
- **THEN** focus moves to the cancel button (cycling within the dialog, not escaping to page elements behind the dialog)

#### Scenario: Focus returns to the trigger on close
- **GIVEN** the dialog was opened by clicking a destructive button
- **WHEN** the dialog closes (via cancel, confirm, Escape, or backdrop click)
- **THEN** keyboard focus returns to the destructive button that opened the dialog

### Requirement: Form-bearing dialogs trap keyboard focus

Every modal dialog component (not only the destructive-confirmation modal) SHALL trap keyboard focus while open: `Tab` from the last focusable element inside the dialog SHALL move focus to the first, and `Shift+Tab` from the first SHALL move focus to the last — focus MUST NOT reach the page content behind the open dialog. The focusable set SHALL be computed at interaction time so elements added or removed by conditional rendering participate correctly. The existing dialog behaviours — initial focus moves into the dialog on open, focus returns to the opener on close, Escape and backdrop dismissal, `role="dialog"`, `aria-modal="true"`, and a labelled title — SHALL be preserved.

#### Scenario: Tab wraps within the open dialog

- **WHEN** a form-bearing modal is open and focus is on its last focusable element and the user presses Tab
- **THEN** focus moves to the dialog's first focusable element, not to the page behind

#### Scenario: Shift+Tab wraps backwards

- **WHEN** the dialog is open with focus on its first focusable element and the user presses Shift+Tab
- **THEN** focus moves to the dialog's last focusable element

#### Scenario: Conditionally rendered fields join the trap

- **WHEN** the dialog's content adds a new focusable element while open (conditional rendering)
- **THEN** subsequent Tab cycles include the new element

#### Scenario: Close restores focus to the opener

- **WHEN** the dialog closes by Escape, backdrop, or an in-dialog action
- **THEN** focus returns to the element that opened it

### Requirement: The modal honours prefers-reduced-motion

`ConfirmDialog` SHALL animate its appearance with a brief enter transition (default: ≤200ms fade + small scale-in for the dialog; backdrop fades only) and a matching leave transition. When the user's environment reports `prefers-reduced-motion: reduce` (CSS media query), both the dialog and backdrop SHALL appear and disappear without transition — the animation duration SHALL be effectively zero, not merely shortened.

This requirement establishes the project's pattern for reduced-motion handling. Future frontend primitives in this capability (and other capabilities) SHALL honour the same media query.

#### Scenario: Default motion in a no-preference environment
- **GIVEN** the user has not requested reduced motion
- **WHEN** the dialog opens
- **THEN** the dialog and backdrop animate in with the default transition

#### Scenario: Reduced motion in a preference-set environment
- **GIVEN** the user's environment reports `prefers-reduced-motion: reduce`
- **WHEN** the dialog opens
- **THEN** the dialog and backdrop appear instantly with no visible transition

### Requirement: The no-JS fallback submits on first click

The confirmation modal is a JavaScript-only enhancement. When JavaScript is unavailable in the client (e.g. progressive-enhancement form actions degraded to full-page POST), the destructive form's submit button SHALL submit the form on the first click without any confirmation step. This matches the pre-confirmation behaviour shipped by `eve-wormhole-mapper-foundation`.

The frontend SHALL NOT build a non-JS confirmation flow (e.g. an intermediate confirmation page rendered server-side). The no-JS path is rare and the destructive backend contract is unchanged — the user's protection in this path is the existing UI affordances (the destructive button is small, red, and not adjacent to a positive action button).

#### Scenario: JavaScript disabled, destructive form submits directly
- **GIVEN** a user with JavaScript disabled visits a page with a destructive form action
- **WHEN** they click the destructive submit button
- **THEN** the form submits as a full-page POST per `use:enhance`'s progressive-enhancement fallback, with no confirmation dialog (because the dialog requires JS)

#### Scenario: No server-rendered confirmation page exists
- **WHEN** the frontend codebase is inspected
- **THEN** there is no route that exists solely to render a server-side confirmation step for a destructive action

### Requirement: A stale-version reload banner prompts users running an outdated UI build

The frontend SHALL detect when the deployed UI version differs from the version the running tab was built with, and SHALL surface a non-destructive prompt offering the user a reload. Detection SHALL use SvelteKit's built-in version mechanism — `kit.version.name` set to the git-derived `APP_VERSION` (the same value the frontend inlines as `PUBLIC_UI_VERSION`; see the `release-versioning` capability) and `updated.current` from `$app/state` — and SHALL NOT hand-roll a polling loop or a custom version endpoint.

`kit.version.pollInterval` SHALL be set to a positive value so staleness is detected in the background (not only on navigation). When `APP_VERSION` is unset at build time (a plain local build), `version.name` SHALL fall back to a fixed, stable string so local development does not spuriously prompt on every rebuild.

The prompt SHALL be a single shared component (canonical path: `frontend/src/lib/components/UpdateBanner.svelte`, per the `sveltekit-node` skill's component-layout rule), mounted once in the root layout (`frontend/src/routes/+layout.svelte`) so it spans all routes. The component SHALL render only when `updated.current` is `true`.

The reload SHALL be user-initiated (`location.reload()` on activating the banner's reload control). The app SHALL NOT reload silently or automatically on navigation or on a timer, so that in-progress, unsaved client state is never discarded without consent. The banner SHALL NOT be a modal overlay that blocks interaction — the user MUST be able to continue working and reload when ready.

All banner copy (the "new version available" message and the reload control label) SHALL be localised via the project's paraglide message system, with keys defined in every supported locale file.

This requirement covers *frontend bundle staleness only*. It SHALL NOT be driven by the backend `/api/health` version or commit — the frontend and backend are separately versioned/deployed artifacts, so backend drift is out of scope for this prompt.

#### Scenario: A newer deployed version triggers the banner
- **GIVEN** a tab running UI build version `A`
- **WHEN** a newer build version `B` has been deployed and SvelteKit's poll observes `B`
- **THEN** `updated.current` becomes `true` and the `UpdateBanner` renders with the localised "new version available" message and a reload control

#### Scenario: Up-to-date tab shows no banner
- **GIVEN** a tab running the currently deployed UI build version
- **WHEN** the version poll runs
- **THEN** `updated.current` is `false` and the `UpdateBanner` does not render

#### Scenario: Reload is user-initiated and not silent
- **GIVEN** the `UpdateBanner` is visible
- **WHEN** the user does NOT activate the reload control
- **THEN** the app does not reload, and the user can continue interacting with the current page
- **AND WHEN** the user activates the reload control
- **THEN** `location.reload()` is invoked, fetching the new bundle

#### Scenario: Local development does not spuriously prompt
- **GIVEN** a local build with no `APP_VERSION` env set
- **WHEN** the app is built and run
- **THEN** `version.name` uses the fixed dev fallback string and the banner does not appear solely due to local rebuilds

#### Scenario: Banner copy is localised
- **WHEN** the `UpdateBanner` renders for a user whose locale is German
- **THEN** the message and reload control are rendered from the German paraglide messages (not hard-coded English)
