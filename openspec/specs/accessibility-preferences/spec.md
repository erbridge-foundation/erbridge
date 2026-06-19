## Purpose

Defines the user-facing accessibility preferences in the SvelteKit frontend: the `/preferences` route, the set of preferences and their defaults, how preferences are applied before first paint (no FOUC), the confirm-or-revert flow for layout-altering changes, and the requirement that all motion respects the reduce-motion preference. Preferences are usable anonymously via `localStorage` and synced to the backend for authenticated users (see the `account-preferences` capability).

## Requirements

### Requirement: /preferences route reachable from the user-menu

The frontend SHALL serve a route at `/preferences` presenting the accessibility preference controls. The user-menu dropdown (`UserMenu.svelte`) `preferences` item SHALL be changed from a disabled `aria-disabled` placeholder into a real enabled `<a href="/preferences">`. The sibling `settings` item SHALL remain a disabled placeholder (out of scope for this change). The `/preferences` route is gated behind authentication: `+layout.server.ts` SHALL NOT treat it as a public route, so an unauthenticated visit redirects to `/login`.

Pre-login accessibility does NOT depend on the `/preferences` route being reachable: the stored preferences are applied before first paint by the inline bootstrap in `app.html` (reading `localStorage`) for every page including `/login`, and the `/login` page itself exposes the accessibility controls (a "maximize accessibility" preset and the locale selector) so a visitor can configure accessibility before or without logging in. When no account is present, controls operate on `localStorage` only.

#### Scenario: preferences link enabled in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** the `preferences` item SHALL be an enabled link to `/preferences` (not `aria-disabled`), and the `settings` item SHALL remain disabled

#### Scenario: Unauthenticated visitor is redirected from /preferences
- **WHEN** a visitor with no session navigates to `/preferences`
- **THEN** the load redirects to `/login` (303); the preference controls are not rendered

#### Scenario: Pre-login accessibility is served without the /preferences route
- **WHEN** a visitor with stored `localStorage` preferences loads `/login`
- **THEN** those preferences are applied to `<html>` before first paint, and the login page's own accessibility controls let the visitor adjust them against `localStorage`

### Requirement: Accessibility preferences and their defaults

The system SHALL provide the following accessibility preferences. Each preference whose default follows an OS media query SHALL default to `auto`, and `auto` SHALL be implemented purely via CSS `@media` with no stored value and no JavaScript override.

- `text_size`: one of `auto`, `small`, `regular`, `large`; default `auto`. Applied by setting `html { font-size }` (`auto`/`regular` ≈ 100%, `small` and `large` scaling down/up). Because all typography is `rem`-relative to `<html>`, this scales the whole UI.
- `reduce_motion`: one of `auto`, `on`, `off`; default `auto`. `auto` follows `@media (prefers-reduced-motion: reduce)`.
- `high_contrast`: one of `auto`, `on`, `off`; default `auto`. `auto` follows `@media (prefers-contrast: more)`.
- `large_targets`: one of `off`, `on`; default `off`. Increases minimum interactive target sizing.
- `dyslexia_font`: one of `off`, `on`; default `off`. Substitutes a dyslexia-friendly typeface for JetBrains Mono.

Non-`auto` overrides SHALL be applied to `<html>`: `text_size` via `font-size`, the remainder via `data-*` attributes (`data-reduce-motion`, `data-high-contrast`, `data-large-targets`, `data-dyslexia-font`) that the CSS keys off.

#### Scenario: Auto reduce-motion follows the OS with nothing stored
- **WHEN** a visitor has `prefers-reduced-motion: reduce` at the OS level and `reduce_motion` is `auto`
- **THEN** motion SHALL be reduced via the CSS `@media` rule with no stored preference and no `data-reduce-motion` attribute

#### Scenario: Explicit override beats the OS default
- **WHEN** `reduce_motion` is set to `off` while the OS prefers reduced motion
- **THEN** `data-reduce-motion="off"` SHALL be applied to `<html>` and motion SHALL NOT be reduced

#### Scenario: Text size scales the UI
- **WHEN** `text_size` is set to `large`
- **THEN** `html { font-size }` SHALL be increased and all `rem`-based typography SHALL scale up accordingly

### Requirement: Preferences apply before first paint (no FOUC)

`app.html` SHALL include an inline `<script>` in `<head>` that, before the body renders, reads the preference bag from `localStorage` and applies any non-`auto` values to `document.documentElement` (the `font-size` and `data-*` attributes). The SvelteKit app SHALL hydrate its preference store from the same source. There SHALL be no visible flash of default styling followed by the preferred styling.

#### Scenario: Stored large text shows immediately on load
- **WHEN** a returning visitor with `text_size: large` in `localStorage` loads any page
- **THEN** the page SHALL render at the large size on first paint, with no flash of the default size

### Requirement: Preference changes are staged and applied as a batch

The `/preferences` page SHALL let the user change one or more preferences as a staged batch before persisting. Changing any control — including `reduce_motion` — SHALL apply the new value as a **live preview** to `<html>` but SHALL NOT persist it. While the staged set differs from the persisted set ("dirty"), the page SHALL present **Apply** and **Discard** controls.

- **Apply** SHALL persist the entire staged batch — to `localStorage`, and synced to the backend for authenticated users — and return the page to a clean (not-dirty) state. There SHALL be no post-apply countdown.
- **Discard** SHALL revert the previews to the persisted values and return to a clean state. It SHALL be shown only while dirty.
- When every control is returned to its persisted value, the page SHALL return to the clean state automatically (no Apply/Discard, nothing to confirm).
- Navigating away while dirty SHALL silently revert the previews to the persisted values, so `<html>` never reflects a value that was not persisted.

The Apply and Discard controls SHALL be styled so a previewed change cannot render them unusable (fixed sizing independent of `text_size`, contrast independent of `high_contrast`).

#### Scenario: Staging multiple changes then applying
- **WHEN** a user changes `text_size` to `large` and `high_contrast` to `on`, then activates **Apply**
- **THEN** both values SHALL be persisted together (localStorage, and PATCHed to the backend for authenticated users) and the page SHALL return to the clean state

#### Scenario: Changes preview but are not persisted before Apply
- **WHEN** a user changes a control but has not activated Apply
- **THEN** the change SHALL be visible as a live preview, AND reloading the page SHALL show the previously persisted value (nothing was persisted)

#### Scenario: Returning to the prior value clears the dirty state
- **WHEN** a user changes `text_size` and then sets it back to its persisted value (with no other control changed)
- **THEN** the page SHALL return to the clean state with no Apply/Discard controls shown

#### Scenario: Discard reverts staged previews
- **WHEN** a user has staged one or more changes and activates **Discard**
- **THEN** the previews SHALL revert to the persisted values and nothing SHALL be persisted

#### Scenario: Navigating away discards unapplied changes
- **WHEN** a user has staged changes and navigates away without applying
- **THEN** the previews SHALL be reverted to the persisted values so the next page reflects the persisted state

#### Scenario: Reduce-motion stages like the others
- **WHEN** a user changes `reduce_motion`
- **THEN** it SHALL be staged as a live preview and persisted only on Apply (it no longer commits immediately)

### Requirement: Reset to defaults is always available as the recovery surface

The `/preferences` page SHALL provide a **Reset to defaults** control that is available in every state (clean or dirty). Activating it SHALL set all five preferences to their default values, apply them to `<html>`, and persist them (localStorage, and synced to the backend for authenticated users).

This control is the lock-out recovery guarantee: because `/preferences` is a robust page reachable from the user menu, and because the Reset control SHALL be styled to remain usable under any applied setting (fixed sizing independent of `text_size`, contrast independent of `high_contrast`), an authenticated user whose applied setting breaks another page can always return to `/preferences` and reset. An anonymous visitor whose `localStorage` setting breaks a page recovers via the equivalent contrast- and size-proof controls on `/login` (which is always reachable without a session). The system SHALL NOT rely on a timed auto-revert for this guarantee.

#### Scenario: Reset restores all defaults
- **WHEN** a user with one or more non-default preferences activates **Reset to defaults**
- **THEN** all five preferences SHALL be set to their defaults, applied to `<html>`, and persisted (localStorage, and PATCHed to the backend for authenticated users)

#### Scenario: Reset is reachable when a setting has broken another page
- **WHEN** an applied preference has made another page hard to use
- **THEN** the user SHALL be able to navigate to `/preferences` and activate **Reset to defaults**, which remains usable because its controls are contrast- and size-proof

### Requirement: All motion respects the reduce-motion preference

Every animation and transition in the frontend SHALL be gated so that it is disabled when motion is reduced — both when `reduce_motion` is `auto` and the OS prefers reduced motion, and when `reduce_motion` is explicitly `on`. This includes (but is not limited to) the pulsing `connected` status dot and character-grid hover transitions.

#### Scenario: Pulsing dot stops under reduced motion
- **WHEN** motion is reduced (via OS default or explicit `on`)
- **THEN** the `connected` status dot SHALL NOT animate

#### Scenario: Hover transitions disabled under reduced motion
- **WHEN** motion is reduced
- **THEN** character-grid hover transitions SHALL be disabled

### Requirement: Native controls render in the dark colour scheme

The frontend SHALL declare `color-scheme: dark` on `:root` so the user agent renders native controls (scrollbars, `<select>` dropdowns, date/number pickers, spinners) and other UA-painted surfaces in their dark variant, matching the app's dark surfaces. The frontend SHALL declare `accent-color: var(--sky)` on `:root` so native checkboxes, radio buttons, and range inputs adopt the theme accent. These declarations apply unconditionally and SHALL NOT introduce a light theme or a user-facing preference.

This is distinct from `prefers-color-scheme`: the app is dark-only by design, so there is no light palette to switch to. `color-scheme: dark` is a browser rendering hint only.

#### Scenario: Native controls match the dark surfaces
- **WHEN** the app renders a native control (scrollbar, `<select>` dropdown, date picker)
- **THEN** the user agent SHALL paint it in its dark variant rather than the light default

#### Scenario: Native form controls use the theme accent
- **WHEN** the app renders a native checkbox, radio button, or range input
- **THEN** the control's accent SHALL be `var(--sky)`

### Requirement: Forced-colors (Windows High Contrast) is supported

The frontend SHALL include an `@media (forced-colors: active)` block in `app.css` that keeps the app usable when the OS forced-colors mode replaces the page palette. This support applies unconditionally; it is NOT a user-facing preference and adds no new preference key, `data-*` attribute, control, or bootstrap behaviour. The existing `high_contrast` preference does not apply in this mode because the user agent overrides author colours.

The `forced-colors: active` block SHALL:
- Restore a visible keyboard-focus indicator using an `outline` (not author colour) on `:focus-visible`, so controls that indicate focus via `outline: none; border-color: var(--sky)` in normal mode remain distinguishable when focused.
- Ensure structural borders that rely on a subtle `--space-*` colour in normal mode use a system colour keyword so they do not vanish.

#### Scenario: Keyboard focus is visible under forced-colors
- **WHEN** forced-colors is active and a keyboard user focuses a control that uses the `outline: none; border-color: var(--sky)` focus pattern
- **THEN** a visible focus indicator (an `outline` using a system colour) SHALL be shown, distinct from the unfocused state

#### Scenario: Structural borders survive forced-colors
- **WHEN** forced-colors is active
- **THEN** borders that are meaningful for layout/separation SHALL remain visible (drawn with a system colour keyword), not collapse into the flattened background

### Requirement: Colour-encoded status signals survive forced-colors

Where colour alone carries meaning — status dots and chips whose state is conveyed by `--emerald` (e.g. connected/success), `--red` (error), or `--amber` (warning) — the frontend SHALL apply `forced-color-adjust: none` to those specific signal elements so the semantic colour is preserved when forced-colors mode would otherwise flatten it. This opt-out SHALL be scoped to the signal elements only; all other elements SHALL honour the user's forced palette. New colour-encoded signal elements added later SHALL be opted out in the same place.

#### Scenario: Connected/error/warning signals keep their colour under forced-colors
- **WHEN** forced-colors is active and a status dot or chip encodes state by colour (connected, error, or warning)
- **THEN** the element SHALL retain its semantic colour via `forced-color-adjust: none` rather than being flattened to a system colour

#### Scenario: Non-signal elements honour the forced palette
- **WHEN** forced-colors is active and an element does not encode meaning by colour
- **THEN** the element SHALL adopt the user's forced system colours (no `forced-color-adjust: none`)

### Requirement: Login page exposes a one-action high-accessibility preset

The login page SHALL present a "Maximize accessibility" control that, when activated, applies a fixed high-accessibility preset through the existing preference store: `text_size: large`, `high_contrast: on`, `reduce_motion: on`, `large_targets: on`, and `dyslexia_font: on`. Activation SHALL take effect immediately (applied to `<html>` so the login page reflects it without a reload) and SHALL persist via the existing store mechanism (localStorage at the edge; no new persistence path). The control SHALL leave `locale` unchanged. The control SHALL be reversible: deactivating it SHALL return those five preference keys to their default values. The control's on/off state SHALL reflect whether the current preference set equals the preset exactly.

#### Scenario: Activating the preset applies all five accessibility preferences
- **WHEN** an unauthenticated user on the login page activates "Maximize accessibility"
- **THEN** the store SHALL commit `text_size: large`, `high_contrast: on`, `reduce_motion: on`, `large_targets: on`, and `dyslexia_font: on`
- **AND** the login page SHALL reflect them immediately (`<html>` font-size and `data-*` attributes set) without a reload
- **AND** `locale` SHALL be unchanged

#### Scenario: Deactivating the preset reverts the five keys to defaults
- **WHEN** the preset is active and the user deactivates "Maximize accessibility"
- **THEN** `text_size`, `high_contrast`, `reduce_motion`, `large_targets`, and `dyslexia_font` SHALL return to their default values
- **AND** the login page SHALL reflect the reverted state immediately

#### Scenario: A first-time user's preset choice is promoted on first sign-in
- **WHEN** a first-time user activates the preset on the login page and then completes sign-in with no prior account preferences
- **THEN** the existing reconciliation SHALL promote the localStorage preferences to the new account (no login-specific persistence path is added)

#### Scenario: An existing user's account preferences win over a login-page tweak
- **WHEN** a returning user with saved account preferences changes settings on the login page and then signs in
- **THEN** their saved account preferences SHALL take precedence on sign-in (intended server-wins reconciliation)
- **AND** the login page guidance SHALL NOT claim the login-page change was saved to their account

### Requirement: Login page exposes a language selector

The login page SHALL present a language selector offering the supported interface locales (`en`, `de`, `fr`). Selecting a locale SHALL commit `locale` through the existing store, which bridges it to the Paraglide locale cookie and re-renders the page in the selected language using the existing translated login strings. The selector SHALL affect only `locale` and SHALL NOT alter the accessibility preferences.

#### Scenario: Selecting a language re-renders the login page in that language
- **WHEN** an unauthenticated user on the login page selects a different supported locale
- **THEN** the store SHALL commit the new `locale`
- **AND** the login page SHALL render its strings (title, subtitle, disclaimer, control labels) in the selected language
- **AND** the accessibility preferences SHALL be unchanged

### Requirement: Login page guides users to post-login preference adjustment

The login page SHALL display informational guidance that the accessibility and language settings can be adjusted after signing in, naming the in-app location (User Menu › Preferences). This guidance SHALL be informational text, NOT a link, because the Preferences destination is reachable only once authenticated. The login-page controls SHALL be presented as applying to the current screen, not as saving to an account.

#### Scenario: Guidance points to the post-login location without linking
- **WHEN** the login page renders
- **THEN** it SHALL show guidance that settings can be adjusted via User Menu › Preferences after signing in
- **AND** that guidance SHALL NOT be a hyperlink

### Requirement: New login-page strings are localized

All new user-facing strings introduced for the login-page accessibility controls (the preset control label and state, the language selector label, and the post-login guidance) SHALL be provided as Paraglide messages in every supported locale (`en`, `de`, `fr`).

#### Scenario: New strings exist in all supported locales
- **WHEN** the login page renders in `en`, `de`, or `fr`
- **THEN** every new control label and guidance string SHALL be present and translated for that locale
