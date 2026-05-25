## Purpose

Defines the user-facing accessibility preferences in the SvelteKit frontend: the `/preferences` route, the set of preferences and their defaults, how preferences are applied before first paint (no FOUC), the confirm-or-revert flow for layout-altering changes, and the requirement that all motion respects the reduce-motion preference. Preferences are usable anonymously via `localStorage` and synced to the backend for authenticated users (see the `account-preferences` capability).

## Requirements

### Requirement: /preferences route reachable from the user-menu

The frontend SHALL serve a route at `/preferences` presenting the accessibility preference controls. The user-menu dropdown (`UserMenu.svelte`) `preferences` item SHALL be changed from a disabled `aria-disabled` placeholder into a real enabled `<a href="/preferences">`. The sibling `settings` item SHALL remain a disabled placeholder (out of scope for this change). The page SHALL be reachable by anonymous visitors (so accessibility can be configured before or without logging in, including from the login flow); when no account is present, controls operate on `localStorage` only.

#### Scenario: preferences link enabled in user-menu
- **WHEN** an authenticated user opens the user-menu dropdown
- **THEN** the `preferences` item SHALL be an enabled link to `/preferences` (not `aria-disabled`), and the `settings` item SHALL remain disabled

#### Scenario: Anonymous visitor reaches /preferences
- **WHEN** a visitor with no session navigates to `/preferences`
- **THEN** the page SHALL render the controls and operate against `localStorage`

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

### Requirement: Layout-altering preference changes auto-revert unless confirmed

Changing a preference that alters visual layout or rendering — `text_size`, `high_contrast`, `large_targets`, and `dyslexia_font` — SHALL apply the change as a live preview and present a confirmation with a countdown. If the user does not confirm within the countdown, the preference SHALL automatically revert to its previous value. The countdown duration SHALL default to 10 seconds and SHALL be defined as a single named constant. `reduce_motion` is excluded from this requirement, because removing motion cannot lock a user out.

The change SHALL be persisted (to `localStorage`, and synced to the backend for authenticated users) ONLY when the user confirms ("Keep"). During the countdown nothing SHALL be persisted, so reloading mid-countdown SHALL show the previous value. The confirmation control SHALL be styled so the previewed change cannot render it unusable (e.g. fixed sizing independent of `text_size`, and a contrast independent of `high_contrast`).

#### Scenario: No action reverts the change
- **WHEN** a user sets `text_size: large` and takes no action for the countdown duration
- **THEN** `text_size` SHALL revert to its previous value and nothing SHALL be persisted

#### Scenario: Keep commits the change
- **WHEN** a user sets `text_size: large` and clicks "Keep" before the countdown ends
- **THEN** `text_size: large` SHALL be written to `localStorage` and, for authenticated users, PATCHed to the backend

#### Scenario: Reload during countdown shows the safe value
- **WHEN** a user sets a layout-altering preference and reloads the page before confirming
- **THEN** the page SHALL load with the previous value, because nothing was persisted during the countdown

#### Scenario: Reduce-motion has no countdown
- **WHEN** a user changes `reduce_motion`
- **THEN** the change SHALL apply and persist immediately with no auto-revert countdown

### Requirement: All motion respects the reduce-motion preference

Every animation and transition in the frontend SHALL be gated so that it is disabled when motion is reduced — both when `reduce_motion` is `auto` and the OS prefers reduced motion, and when `reduce_motion` is explicitly `on`. This includes (but is not limited to) the pulsing `connected` status dot and character-grid hover transitions.

#### Scenario: Pulsing dot stops under reduced motion
- **WHEN** motion is reduced (via OS default or explicit `on`)
- **THEN** the `connected` status dot SHALL NOT animate

#### Scenario: Hover transitions disabled under reduced motion
- **WHEN** motion is reduced
- **THEN** character-grid hover transitions SHALL be disabled
