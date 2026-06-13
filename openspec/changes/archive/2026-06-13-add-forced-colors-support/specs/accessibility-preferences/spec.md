## ADDED Requirements

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
