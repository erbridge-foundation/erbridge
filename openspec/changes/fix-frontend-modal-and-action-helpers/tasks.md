# Tasks — fix-frontend-modal-and-action-helpers

## 1. Modal focus trap

- [ ] 1.1 Add Tab/Shift+Tab trapping to `Modal.svelte` (focusable set computed per keypress, wrap both directions); keep initial-focus, restore-on-close, Escape, backdrop, and reduce-motion behaviour unchanged
- [ ] 1.2 Vitest: Tab wrap, Shift+Tab wrap, conditionally-added field joins the cycle, focus restore on close

## 2. Shared action-error helper

- [ ] 2.1 Add `$lib/form-errors.ts` with `failFrom(action, e, extra?)` reproducing the existing payload shapes exactly; unit tests for ApiError and unknown-error branches incl. extras passthrough
- [ ] 2.2 Replace the catch blocks in `maps/+page.server.ts`, `maps/[slug]/settings/+page.server.ts`, and `acls/[id]/+page.server.ts`; existing action Vitest suites must pass without assertion changes

## 3. MemberPicker state hygiene

- [ ] 3.1 Reset `perms` when a new result set arrives; Vitest covering reset-on-new-results and default-to-read after reset

## 4. Verification

- [ ] 4.1 `pnpm --filter frontend test` — Vitest unit/component tests
- [ ] 4.2 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile)
- [ ] 4.3 `pnpm --filter frontend run test:e2e` — Playwright e2e tests
- [ ] 4.4 Manual keyboard pass on the create-map modal and member picker: Tab cannot escape the open dialog; screen-reader smoke (dialog announced with title)
