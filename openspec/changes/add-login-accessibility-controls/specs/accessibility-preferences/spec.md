## ADDED Requirements

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
