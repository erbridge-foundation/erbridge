## Context

The foundation change shipped three destructive actions on `/characters` (`remove`, `delete account`, and the link-based `re-auth`) as single-click form submissions. The proposal explains why we want a confirmation step. This document records the design decisions reached during `/opsx:explore confirm-destructive-actions` — chiefly: this is built as a reusable primitive, not a one-off, because the destructive-action surface is about to expand.

The exploration enumerated three plausible patterns (native `window.confirm()`, inline two-step button, modal dialog) and several axes of choice (component API shape, copy structure, action-coverage policy, accessibility behaviour, motion behaviour). This document captures the choices made and the reasoning, so future changes that touch the modal — or want an exception to it — have the trade-off context they need.

## Goals / Non-Goals

**Goals:**

- A single shared `ConfirmDialog` component used by every destructive action in the SvelteKit frontend.
- A copy contract — title, body, confirm-label structure — that future destructive features inherit instead of re-inventing.
- A coverage policy stated as a mechanical test ("does this action mutate server state without a symmetric undo?"), not a per-feature judgement call.
- Accessibility behaviour (alertdialog semantics, focus trap, Escape, default focus on cancel, reduced-motion honouring) baked into the primitive so callers cannot accidentally regress it.
- Zero backend changes. Confirmation is a frontend concern; the existing form-action contract is the source of truth for what actually happens server-side.
- A no-JS fallback that submits the form on the first click (today's behaviour). Confirmation is a JS-only enhancement, not part of the form contract.

**Non-Goals:**

- An imperative `await confirm({ ... })` API. Tempting for terseness; rejected because it requires a portal/mount-anywhere root, forces every body to be plain text or HTML strings (no rich markup), and trades a thin ergonomic win for a heavier architecture. Snippet props give the same call-site terseness in the no-formatting case and full composition when bodies want bold names or lists.
- A `<ConfirmableForm>` wrapper component that bundles `use:enhance` + the dialog. Rejected because future callers (ACL row removals, possibly AJAX-driven rather than progressive-enhancement forms) need to drive their own submission path. The primitive stays form-agnostic.
- Type-to-confirm (e.g. "type your account name to enable the button"). Rejected for v1 because the modal already removes the misclick risk, and type-to-confirm is enterprise-grade friction that doesn't fit this app's tone. Worth reconsidering only if real users report accidental confirmations.
- Per-action toast / undo affordances ("character removed — undo"). Out of scope. The modal IS the undo opportunity, taken before the destruction. A toast-based undo would change the server contract (a soft-delete with a delayed commit) and is a much larger discussion.
- A second `tone` (e.g. `"warning"`, `"info"`). The modal is v1 destructive-only; the API accepts a `tone` prop so future non-destructive uses can be added without a breaking change, but the only supported value in this change is `"danger"`.
- Generalising the modal to a generic `<Dialog>` primitive. The confirmation modal is **not** a generic dialog — it has a specific shape (title, body, two buttons, cancel-by-default) that encodes the copy contract. If we later need a generic dialog (forms-in-dialogs, multi-step flows), it is a separate primitive, not a generalisation of this one.

## Decisions

### Decision 1: Component API — Svelte 5 snippet props (B), not pure props (A) or imperative (C)

The exploration laid out three component-API shapes:

```
A (pure props):     <ConfirmDialog title="..." body="..." confirmLabel="..." />
B (snippet props):  <ConfirmDialog>{#snippet title()}Remove {name}?{/snippet}...
C (imperative):     if (await confirmAction({ title, body, confirmLabel }))
```

We picked **B**. The deciding factor is ACL-class callers wanting rich body markup (e.g. "Remove **CCP Falcon** from this ACL?" with the name bold-typed). Option A pushes those callers to HTML strings (innerHTML hazard); option C has the same problem and additionally requires a portal mount. Snippet props give:

- Plain-text terseness in the simple case: `{#snippet title()}Delete account?{/snippet}` is one line.
- Full Svelte composition (bold, lists, links, interpolated names) without `{@html}` escapes.
- Type safety via Svelte 5's typed snippet props.

The cost is verbosity vs. option A in the no-formatting case (~3 extra lines per caller). Acceptable given the upside.

### Decision 2: Cancel is the default-focused button (not confirm)

When the modal opens, focus lands on the **cancel** button. Reason: this protects against a user who hits `Enter` reflexively after the modal appears — `Enter` confirms the focused button, and we want that default to be the safe choice. GitHub puts focus on confirm; Stripe puts focus on cancel; we go Stripe. The cost is one extra Tab keystroke for users who genuinely want to confirm; the upside is no `Enter`-storms triggering accidental destruction.

Escape also cancels (per `alertdialog` convention). There is no keyboard shortcut for confirm — a deliberate keystroke on the confirm button is required.

### Decision 3: Strict action-coverage policy (A from exploration), not a carve-out (B) or deferred (C)

The policy: every frontend action that mutates server state AND is not reversible by a symmetric undo SHALL use the modal. Symmetric undo means a UI action that produces the inverse state with no data loss (e.g. "set main: A" can be undone by "set main: B"; "remove character A" cannot be undone by anything in-UI — it requires re-running SSO).

A future change that believes its destructive action deserves a different pattern (e.g. inline two-step for high-frequency low-stakes actions like "remove one of fifty ACL entries") MUST propose the exception in its own change. The exception goes through the same review as any other spec change; it does not happen by default.

The trade-off is real: ACL-row removals may want an inline pattern, and forcing them through this policy means the ACL change either accepts the modal or opens a spec amendment. We chose strictness because the cost of a too-permissive policy (every change re-litigates the pattern) is higher than the cost of one or two future exception proposals. The exception process is light: a few paragraphs in the change's proposal, no code churn here.

### Decision 4: Copy structure is a contract, not a suggestion

The spec defines the copy structure as a SHALL, not a guideline:

- **Title**: `<destructive verb> <object>?` — e.g. `Delete map?`, `Remove character?`, `Delete account?`.
- **Body**: one sentence, present tense, describing the consequence in user-visible terms. Use the actual name where helpful (e.g. "Bookmarks and the map will be permanently removed."). No "this will" or "are you sure".
- **Confirm label**: the destructive verb echoing the action — e.g. `delete map`, `remove character`, `delete account`. Never generic `confirm` / `yes` / `OK`. Echoing reduces muscle-memory misclicks: a user who has trained their hand to click "OK" on dialogs has to read the actual word before clicking.
- **Cancel label**: `cancel`, always.

The cost of the contract is per-caller copywriting effort. The upside is that the user sees a consistent vocabulary across every destructive action in the app — once they have read one confirmation, the next one is shaped exactly the same.

### Decision 5: Spec home — new `frontend-patterns` capability, not extending `account-management`

The exploration considered three homes for the requirement: (A) extend `account-management` with a frontend section, (B) create a new `frontend-patterns` capability, (C) defer the question. With multiple future callers (maps, ACLs, etc.) now known, A doesn't fit — the modal is above any one capability — and C creates the same churn we're trying to avoid. We picked B.

Naming: `frontend-patterns` is intentionally broad. The alternatives (`destructive-actions`, `confirmation-ui`) are more precise but pre-commit the capability to one purpose. The cost of one too-broad-named spec is far lower than the cost of three single-purpose specs each holding one component. The next shared frontend primitive (toasts? form-error rendering? loading states?) can land here without a new capability.

### Decision 6: The 30-day grace period IS named in the `delete account` copy

The hard-delete-after-30-days process is a future change that hasn't been designed yet. The soft-delete itself already exists. Two options for the copy:

```
Option 1 — name the number
  "Your account will be deactivated. To restore it, log back in
   within 30 days; after that, your data is permanently removed."

Option 2 — describe behaviour, defer the number
  "Your account will be deactivated. To restore it, log back in
   with any of your characters."
```

We picked Option 1. Users care about the horizon, not the abstract behaviour. "Will be deactivated" with no time bound is more confusing than "30 days, then gone." If the future hard-delete change picks 14 days or 60, we update one string here — and that change is required to do so as part of its tasks.

Assumption captured: the future hard-delete change SHALL update this copy to match its chosen grace period. The relevant string lives in `frontend/src/routes/characters/+page.svelte` (or whatever file holds the `delete account` modal copy at the time).

### Decision 7: Motion respects `prefers-reduced-motion`

Default transition: 150ms fade + small scale-in (0.96 → 1.0) for the dialog; the backdrop fades only. When `prefers-reduced-motion: reduce` is set, both the dialog and backdrop appear instantly with no transition.

This decision is small in scope here but establishes a pattern: the separate `accessibility-preferences` proposal leans into the same media query for other preferences. Establishing the pattern in this change means that one inherits it for free.

### Decision 8: The fallback when JS is disabled is "submit on first click"

`use:enhance` already degrades to a full-page form POST when JS is unavailable. The modal is a JS-only enhancement; with no JS, the form's submit button submits on the first click — i.e. the pre-foundation behaviour. We accept this fallback explicitly: the no-JS path is rare, the destructive actions still work (the backend contract is unchanged), and the user's protection from misclicks is "the destructive button is small and red and not adjacent to a positive action button" — the same protection that existed before this change.

This is a non-goal: we do NOT build a no-JS confirmation path (e.g. a two-step form that posts to an intermediate confirmation page). The complexity cost is not justified by the no-JS population for this app.

## Risks / Trade-offs

- **Risk: the action-coverage policy gets gamed.** A change author who wants to skip the modal for a borderline case may write a proposal that argues the action has a "symmetric undo" when it doesn't. Mitigation: the policy's wording ("symmetric undo means UI action that produces the inverse state with no data loss") is concrete enough that gaming requires an obvious stretch, which review will catch.

- **Trade-off: snippet-based API has more boilerplate than pure props.** Simple call sites pay ~3 extra lines vs. `<ConfirmDialog title="..." body="..." />`. Accepted in exchange for rich-body support without `{@html}`.

- **Risk: per-action copywriting is required for every new destructive feature.** Each future destructive feature must write title/body/confirm-label copy, which is a small but real cost. Mitigation: the structured contract makes the copy mechanical (verb + object pattern); the cost is minutes, not hours.

- **Trade-off: cancel-default focus costs one Tab for confirmation-intent users.** Accepted because the protective value (no `Enter`-storm misclicks) exceeds the friction.

- **Risk: the 30-day copy decouples from the hard-delete implementation.** If the hard-delete change ships with a different grace period and forgets to update this copy, users see a false promise. Mitigation: an explicit assumption in this design.md plus a tasks.md entry in the future hard-delete change to update the string. If the future change misses it, it shows up in code review as a stale magic number — annoying but not catastrophic.

- **Trade-off: building the modal as a primitive on day one (with two callers) is more work than wiring two ad hoc confirmations.** Estimated additional cost: one extra day of design + tests for the primitive, plus the spec. Justified by the 5+ known future callers (delete map, delete ACL, remove character/corp/alliance from ACL, etc.) — even one re-use pays for the up-front investment.

- **Risk: `frontend-patterns` becomes a junk drawer.** A broad-named capability can accumulate unrelated primitives over time. Mitigation: when the third primitive lands here, evaluate whether to split (e.g. `frontend-dialogs`, `frontend-feedback`). For now the capability has exactly one primitive and the breadth is a feature, not a bug.

## Open Questions

- **None blocking.** The decisions above cover every axis the exploration surfaced. The one item that is genuinely deferred — whether ACL-row removal eventually wants a different pattern — is intentionally deferred via the strict-policy-plus-exception-process design (Decision 3).
