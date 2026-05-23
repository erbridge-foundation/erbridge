## Why

The `eve-wormhole-mapper-foundation` change ships three destructive actions on the `/characters` page — `remove` (deletes a non-main character), `delete account` (soft-deletes the account), and indirectly `re-auth` (replaces stored tokens) — each wired as a single-click form submission with no confirmation step. A user who clicks `delete account` by reflex has no recovery path inside the UI; a user who clicks `remove` on the wrong card cannot undo it without re-running EVE SSO. The wireframes were signed off without a confirmation pattern because choosing one belonged in a focused conversation, not buried in the foundation.

This change introduces that confirmation pattern. The pattern is built **as a reusable primitive**, not a one-off, because the destructive-action surface is about to expand: future changes ship destructive map actions (delete map), destructive ACL actions (remove character/corp/alliance from ACL), and likely more. Building the modal once — with a structured copy contract, accessibility baked in, and a stated coverage policy — lets every later change inherit the contract instead of re-litigating it.

The change also fills a small gap in the spec layout: there is no `frontend-patterns` capability yet. Adding one now (with a single requirement to start: the confirmation modal) gives a natural home for the next shared frontend primitive without forcing a refactor of an existing capability.

## What Changes

- New shared component `frontend/src/lib/components/ConfirmDialog.svelte` implementing a modal confirmation dialog. The component takes Svelte 5 snippet props (`title`, `body`, `confirmLabel`) so callers can pass rich markup (bold names, lists, links) without an `innerHTML` escape hatch, plus plain props for `open`, `tone` (`"danger"` for v1; the only tone), `onCancel`, and `onConfirm`. The component owns its DOM and accessibility (`role="alertdialog"`, focus trap, `Escape` to cancel, default focus on **cancel**), but does NOT own submission — callers wire `onConfirm` to whatever they need (e.g. `formEl.requestSubmit()` for `use:enhance` forms, or a fetch call for AJAX-driven actions).
- Wire the modal into the two existing destructive actions on `/characters`:
  - `?/remove` — title `Remove <name>?`, body explaining that the character's stored tokens are removed and re-adding requires a fresh SSO flow, confirm label `remove character`.
  - `?/deleteAccount` — title `Delete account?`, body explaining the account is deactivated and can be restored by logging back in within 30 days (after which data is permanently removed), confirm label `delete account`. The 30-day grace is named because users care about the horizon; the actual hard-delete process is a future change but the soft-delete itself already exists, so the copy is forward-compatible.
- Adopt a **structured copy contract** for destructive confirmations (title = `<verb> <object>?`, body = consequence in one present-tense sentence using the actual name where helpful, confirm label = the destructive verb echoing the action — never generic `confirm` / `yes`, cancel label = `cancel` always). Capturing this in the spec means future destructive features don't re-invent copy from scratch.
- Adopt a **strict action-coverage policy**: every frontend action that mutates server state AND is not reversible by a symmetric undo (e.g. "set main" is symmetric — picking someone else undoes it; "remove character" is not — re-running SSO is not a symmetric undo) SHALL go through the modal. Navigation/redirect-only actions (`re-auth`, `log out`) and actions with built-in undo MAY skip it. A future change that wants to use a different pattern (e.g. inline two-step for high-frequency low-stakes actions) MUST propose an exception in its own change rather than degrade the policy here.
- Honour `prefers-reduced-motion` in the modal's enter/leave transition (~150ms fade+scale by default; instant for users who request reduced motion). The `accessibility-preferences` change (separately proposed) will lean into the same media query — establishing the pattern here means that change inherits it.
- Update `frontend/wireframes/characters.html` to reflect the modal in both its open state (one variant for `remove`, one for `delete account`) and its closed state (the buttons render the same as today). The live Svelte file is authoritative post-archival; the wireframe update is for consistency, not as a design source.
- No backend changes. The confirmation is purely client-side; the existing form actions and 4xx error envelope continue to handle the actual destruction. The progressive-enhancement fallback (no-JS) submits the form on the first click as today — confirmation is a JS-only enhancement, not a contract.

## Capabilities

### New Capabilities

- `frontend-patterns`: shared SvelteKit-frontend primitives that span multiple capabilities. Initial scope: the destructive-action confirmation modal, the structured-copy contract for destructive confirmations, the strict action-coverage policy, and the accessibility requirements (`alertdialog`, focus trap, Escape, default focus on cancel, reduced-motion honouring). The capability is intentionally broad-named so future shared frontend primitives (toasts, form-error rendering, loading states) can live here without spinning up a new capability per primitive.

### Modified Capabilities

None. `account-management` remains a pure HTTP-contract capability; the confirmation is a frontend concern and lives in `frontend-patterns`. Existing destructive endpoints (`DELETE /api/v1/characters/:id`, `DELETE /api/v1/account`) are unaffected — they still return the same status codes and error envelope.

## Impact

- **Frontend code**: one new component `frontend/src/lib/components/ConfirmDialog.svelte` (~150 LOC including styles); modifications to `frontend/src/routes/characters/+page.svelte` to gate the two destructive forms behind the modal (forms become `type="button"` triggers that open the modal; the modal's `onConfirm` calls `formEl.requestSubmit()` so `use:enhance` and the existing error-rendering path are unchanged).
- **Tests**: unit-level Svelte tests for `ConfirmDialog` (rendering, focus management, Escape cancels, Enter on cancel button cancels, `onConfirm` fires only on confirm click); update to `frontend/src/routes/characters/page.server.test.ts` is NOT needed because the confirmation is client-side and the server action's behaviour is unchanged — but a new browser-level / Playwright-style test (if/when that harness exists; otherwise a documented manual verification step) confirms the modal gate works end-to-end.
- **Wireframes**: `frontend/wireframes/characters.html` gains two open-state variants (one per destructive action). Foundation task 7.29 has already moved the wireframes folder under `frontend/`, so no path migration is needed.
- **Spec layout**: a new `openspec/specs/frontend-patterns/` capability folder is added. No existing spec is touched.
- **Risk**: very low. The change is additive on the client; the no-JS fallback path is unchanged; the backend contract is unchanged; nothing depends on the absence of a confirmation step.
- **Dependency on foundation**: this change does not modify any spec introduced by foundation — it only adds a new capability and modifies frontend code that foundation shipped. It can be proposed and archived independently after foundation is archived.
- **Forward-compatibility coupling**: the `delete account` copy names "30 days" as the grace period before hard delete. The hard-delete process is a future change; if that change picks a different number, this change's copy MUST be updated. Captured as an explicit assumption in `design.md`.
