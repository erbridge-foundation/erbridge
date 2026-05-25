## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Reset to defaults is always available as the recovery surface

The `/preferences` page SHALL provide a **Reset to defaults** control that is available in every state (clean or dirty). Activating it SHALL set all five preferences to their default values, apply them to `<html>`, and persist them (localStorage, and synced to the backend for authenticated users).

This control is the lock-out recovery guarantee: because `/preferences` is a robust page reachable from the user menu, and because the Reset control SHALL be styled to remain usable under any applied setting (fixed sizing independent of `text_size`, contrast independent of `high_contrast`), a user whose applied setting breaks another page can always return to `/preferences` and reset. The system SHALL NOT rely on a timed auto-revert for this guarantee.

#### Scenario: Reset restores all defaults
- **WHEN** a user with one or more non-default preferences activates **Reset to defaults**
- **THEN** all five preferences SHALL be set to their defaults, applied to `<html>`, and persisted (localStorage, and PATCHed to the backend for authenticated users)

#### Scenario: Reset is reachable when a setting has broken another page
- **WHEN** an applied preference has made another page hard to use
- **THEN** the user SHALL be able to navigate to `/preferences` and activate **Reset to defaults**, which remains usable because its controls are contrast- and size-proof
