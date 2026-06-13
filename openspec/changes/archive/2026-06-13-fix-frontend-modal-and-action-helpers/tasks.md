# Tasks — fix-frontend-modal-and-action-helpers

## 1. Modal focus trap

- [x] 1.1 Add Tab/Shift+Tab trapping to `Modal.svelte` (focusable set computed per keypress, wrap both directions); keep initial-focus, restore-on-close, Escape, backdrop, and reduce-motion behaviour unchanged
- [x] 1.2 Vitest: Tab wrap, Shift+Tab wrap, conditionally-added field joins the cycle, focus restore on close

## 2. Shared action-error helper

- [x] 2.1 Add `$lib/form-errors.ts` with `failFrom(action, e, extra?)` reproducing the existing payload shapes exactly; unit tests for ApiError and unknown-error branches incl. extras passthrough
- [x] 2.2 Replace the catch blocks in `maps/+page.server.ts`, `maps/[slug]/settings/+page.server.ts`, and `acls/[id]/+page.server.ts`; existing action Vitest suites must pass without assertion changes

## 3. MemberPicker state hygiene

- [x] 3.1 Reset `perms` when a new result set arrives; Vitest covering reset-on-new-results and default-to-read after reset

## 4. Verification

- [x] 4.1 `pnpm test` (from `frontend/`) — Vitest unit/component tests — 306/306 pass
- [x] 4.2 `pnpm run check` (from `frontend/`) — svelte-check — 0 errors / 0 warnings
- [x] 4.3 `pnpm run test:e2e` (from `frontend/`) — Playwright e2e tests — 25/25 pass
- [x] 4.4 Manual keyboard pass on the create-map modal and member picker: Tab cannot escape the open dialog (verified); dialog/picker focus behaviour confirmed by hand. Screen-reader smoke deferred (no SR available to tester); semantics (`role="dialog"`, `aria-modal`, `aria-labelledby`) are unit-asserted.
  - Manual pass surfaced two missing focus indicators (out of the trap's own scope but found during it, folded in here): the create-map "default ACL" checkbox and the member-picker scope radios had no visible focus ring. Added `:focus-visible` outlines (and `accent-color`) to both so keyboard focus is visible on these native controls.
