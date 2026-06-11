# Fix Frontend Modal and Action Helpers

## Why

A frontend review (2026-06-11) of the maps/ACLs UI found that the new form-bearing `Modal.svelte` does not trap Tab — it handles Escape, backdrop dismissal, initial focus, and focus restore, but keyboard focus can walk out of the open dialog into the obscured page, undermining its `aria-modal="true"` claim (the sibling `ConfirmDialog` traps correctly). Secondarily, the `try/catch → ApiError → fail(...)` block is duplicated ~10 times across the maps/ACLs form actions, and `MemberPicker`'s per-result permission state accumulates across searches.

## What Changes

- `Modal.svelte` gains a focus trap: Tab/Shift+Tab cycle within the dialog's focusable elements while open, matching the behaviour (not necessarily the two-element implementation) of `ConfirmDialog`.
- A shared `failFrom(action, e, extra?)` helper in `$lib` replaces the duplicated catch blocks in `maps/+page.server.ts`, `maps/[slug]/settings/+page.server.ts`, and `acls/[id]/+page.server.ts`, preserving the exact `fail` payload shape (`action`, `code`, `message`, optional extras).
- `MemberPicker` resets its per-result permission selections when a new search's results arrive.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `frontend-patterns`: form-bearing dialogs join the modal accessibility contract — a focus-trap requirement covering generic dialogs, not only the destructive-confirmation modal.

The helper extraction and picker-state reset are implementation-only (no requirement change).

## Impact

- Frontend only: `Modal.svelte`, new `$lib/form-errors.ts` (or similar), three `+page.server.ts` action files, `MemberPicker.svelte`; Vitest suites for each.
- No backend, schema, or i18n changes.
