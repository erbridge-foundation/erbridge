# Design — fix-frontend-modal-and-action-helpers

## Context

`Modal.svelte` was introduced as the form-bearing sibling of `ConfirmDialog` (whose two-focusable trap doesn't generalise to arbitrary form content). It moves initial focus in and restores focus on close, but a Tab press from the last field exits into the backdrop-obscured page. The three maps/ACLs action files each repeat the same `catch (e) { if (e instanceof ApiError) return fail(e.status, {...}); return fail(500, {...}) }` block with only the `action` tag and extras varying. `MemberPicker.perms` is keyed by result id and never pruned, so selections leak across unrelated searches (harmless at 25-result scale, but trivially fixed).

## Goals / Non-Goals

**Goals:**
- Keyboard focus cannot leave an open `Modal`; behaviour matches WAI-ARIA dialog expectations.
- One catch-block implementation; payload shapes byte-identical to today so no `+page.svelte` or test assertions change semantically.

**Non-Goals:**
- Migrating `ConfirmDialog` onto the generic trap (it works; consolidation can ride a later touch of that file).
- Adopting `<dialog>`/`showModal()` (native top-layer + trap) — considered, but it changes styling/transition behaviour and the project's modal pattern is established; revisit wholesale, not piecemeal.
- New error UX; this is plumbing only.

## Decisions

**Generic trap via keydown interception.** On `Tab`/`Shift+Tab` inside the dialog, compute the focusable set (`a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])`, filtered for visibility) at keypress time — not cached — so fields added/removed by conditional rendering are always included. Wrap from last→first and first→last. The existing initial-focus `$effect` stays. Alternative considered: sentinel focus guards (invisible focusable spans bracketing the dialog) — workable but adds DOM and tab-order subtlety; the keydown approach is self-contained and unit-testable with Testing Library.

**`failFrom(action, e, extra?)` in `$lib/form-errors.ts`.** Signature: `failFrom(action: string, e: unknown, extra?: Record<string, unknown>)` returning the `fail(...)` result — `ApiError` → `fail(e.status, { action, code: e.code, message: e.message, ...extra })`; anything else → `fail(500, { action, code: 'internal_error', message: 'An unexpected error occurred', ...extra })`. Server-only usage from `+page.server.ts` files; it imports only `@sveltejs/kit`'s `fail`, so `$lib` placement is fine. Call sites keep their `try` and validation logic — only the catch body shrinks.

**Picker state reset keyed on result identity.** `MemberPicker` clears `perms` in an `$effect` watching the incoming result props (reset when the result set changes). Defaulting (`permFor` → `'read'`) already handles the cleared state.

## Risks / Trade-offs

- [Trap vs browser chrome] The trap intercepts Tab only; Ctrl/Cmd+L, F6 etc. still reach the browser — correct per ARIA practices (trap document focus, never browser chrome).
- [Focusable-set queries on each Tab] Negligible cost at dialog scale; avoids stale-cache bugs.
- [Behavioural snapshot risk in actions] The helper must reproduce payloads exactly; the existing Vitest action suites (which assert `fail` payload shapes) are the guard — they should pass without assertion edits.

## Migration Plan

Frontend-only deploy; no migration. Rollback is a revert.
