# frontend-patterns — delta for fix-frontend-modal-and-action-helpers

## ADDED Requirements

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
