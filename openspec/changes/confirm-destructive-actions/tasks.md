## 0. Required skills

Before implementing tasks, the implementer MUST invoke the skill matching the area being worked on. Each skill defines mandatory architecture, structure, and convention rules.

| Working on… | Skill | Invoke when |
|---|---|---|
| Anything under `frontend/` (sections 2, 3, 4) | `sveltekit-node` | Before writing the first line of Svelte / TypeScript in `frontend/` in this session. §5 wireframe is plain HTML and does NOT require this skill, but it must be approved before any visual tweaks land in the Svelte component. |

There are no backend tasks in this change. The confirmation is purely client-side; the existing form-action contract and 4xx error envelope are unchanged.

## 0a. Prerequisite

This change depends on `eve-wormhole-mapper-foundation` being archived first, because it modifies `frontend/src/routes/characters/+page.svelte` which is introduced by foundation. It does NOT depend on any other in-flight change.

## 1. Component contract (decide before building)

- [x] 1.1 Confirm the `ConfirmDialog` prop signature matches the spec exactly (`open`, `tone: "danger"`, `title: Snippet`, `body: Snippet`, `confirmLabel: Snippet`, `onCancel`, `onConfirm`). If any deviation is needed during implementation, update `specs/frontend-patterns/spec.md` first — the spec is the source of truth.
- [x] 1.2 Decide the cancel label string used everywhere in the codebase: `cancel` (lowercase, no trailing space). The label is hard-coded inside the component, not a prop — the spec requires every invocation to use the same word. Document this decision as a one-line comment at the top of the component.

## 2. Frontend: ConfirmDialog component

- [x] 2.1 Add `frontend/src/lib/components/ConfirmDialog.svelte` per the `sveltekit-node` skill's component-layout rule. Implement the API in §1.1 using Svelte 5 syntax (`$props`, `$state`, `$effect`). The component file SHALL contain no business logic — it is pure presentation + accessibility + motion.
- [x] 2.2 Implement the DOM structure inside a `<dialog>` element (or a `<div role="alertdialog">` with manual modality if `<dialog>`'s open-state quirks bite). Whichever element is used, the root SHALL have `role="alertdialog"`, `aria-modal="true"`, `aria-labelledby` pointing to the title element's id, and `aria-describedby` pointing to the body element's id. Title and body element ids SHALL be generated per-instance (Svelte 5's `$props.id()` or a simple `crypto.randomUUID()` slice) so multiple dialogs on a page never collide.
- [x] 2.3 Implement focus management:
  - On open (`$effect` watching `open`), capture the previously-focused element (`document.activeElement`) and move focus to the cancel button.
  - While open, trap focus by listening for `Tab` / `Shift+Tab` on the dialog root and cycling between the two focusable buttons (cancel, destructive).
  - On close (any path: cancel click, confirm click, Escape, backdrop click), restore focus to the previously-focused element.
  - On `Escape` keydown, invoke `onCancel`.
- [x] 2.4 Implement backdrop click to cancel: a backdrop element absolutely-positioned behind the dialog body listens for click and invokes `onCancel`. Clicks inside the dialog body SHALL NOT bubble to the backdrop (stop propagation on the dialog body's pointerdown / click).
- [x] 2.5 Implement the motion: default enter is ~150ms (use a design-token-friendly value if `frontend/src/lib/tokens` exposes durations; otherwise hard-code 150ms with a TODO referencing the design-token system). Use Svelte's built-in `fade` for backdrop and `scale` for dialog body, with `start: 0.96`. Honour `prefers-reduced-motion` via a CSS `@media (prefers-reduced-motion: reduce)` block that sets `animation-duration: 0s` and `transition-duration: 0s` on both elements, AND by passing `duration: 0` to the Svelte transitions when the media query matches (matched at runtime via `window.matchMedia('(prefers-reduced-motion: reduce)').matches`). Both paths are required because the JS path covers Svelte's own transition machinery and the CSS path covers any non-Svelte animations a future contributor might add.
- [x] 2.6 Styles: use the project's design tokens (`var(--space-…)`, `var(--slate-…)`, `var(--red)`) per the `sveltekit-node` skill's design-token rule. The destructive button SHALL use the existing `--red` token (matching the `.danger-btn` colour in the current `/characters` page). The cancel button SHALL be visually subordinate (no fill, slate text). The backdrop SHALL be a low-opacity dark overlay (e.g. `rgba(0, 0, 0, 0.5)`). The dialog body SHALL be responsive: full-width with safe-area padding below 600px (matching the existing `/characters` breakpoint), centered with a fixed max-width above 600px.
- [x] 2.7 Mount target: the dialog SHALL render at the document root level (not inside its caller's DOM tree) to avoid `overflow: hidden` ancestors clipping the dialog. Implement via `{#if open}` at the component root with a `position: fixed` root style — no portal library needed.

- [x] 2.8 **Handoff contract for §3.** Section 2 will be implemented by a different model than sections 3–5. To let §3 treat the component as a black box (and to let §3's implementer verify the wiring works without re-reading the component internals), §2's implementer SHALL produce the following before §3 begins:
  - **A frozen prop signature**, as a TypeScript declaration block at the top of `ConfirmDialog.svelte`'s `<script lang="ts">`. Any deviation from the §1.1 / spec signature SHALL have been resolved by amending the spec first (per §1.1), not by silently differing here.
  - **A short usage example** as a multi-line comment at the top of the component file, showing the exact snippet syntax §3 should use to mount the dialog and call `requestSubmit()` on confirm. This is the only documentation §3's implementer is expected to read.
  - **A list of confirmed behaviours**, captured as a comment block immediately above the prop signature. Each behaviour is tagged either `@verified-by-test:` (covered by the §4.1 Vitest suite) or `@needs-browser-check:` (cannot be reliably verified in jsdom; deferred to §6 in a real browser). The eight behaviours that MUST appear in some form are:
    - opens with cancel focused
    - Escape calls onCancel
    - backdrop click calls onCancel
    - clicks inside dialog body do not bubble to backdrop
    - Tab cycles between cancel and confirm; Shift+Tab cycles back
    - onConfirm fires only on confirm button activation (click or Enter)
    - focus returns to the opening element on close
    - prefers-reduced-motion: reduce disables both Svelte transitions and CSS animations

  §3 SHALL NOT proceed if any of the eight items is missing entirely. Items tagged `@needs-browser-check:` SHALL also be enumerated in §6 (verification) so they are not lost. The block is a one-time handoff aid — it MAY be removed in a follow-up cleanup after the change is archived.
  - **The §4.1 test file SHALL exist and pass** before §3 begins. Even though §4 is nominally "after §3" in section ordering, the test suite is the second pillar of the handoff contract: if §3's implementer needs to debug a wiring problem, the failing test gives them a localised signal rather than a "the modal feels broken" symptom in the live page.

## 3. Frontend: wire /characters destructive actions through the modal

- [x] 3.1 Edit `frontend/src/routes/characters/+page.svelte`: import `ConfirmDialog` and introduce per-card and page-level confirmation state. For the per-character `remove` flow, use a single state pair `{ open: boolean, character: Character | null }` so the modal can render the active character's name in the title. For `deleteAccount`, a separate boolean `deleteAccountOpen` is sufficient.
- [x] 3.2 Convert the `remove` form: keep the `<form method="POST" action="?/remove" use:enhance>` and hidden `character_id` input, but change the submit button to `type="button"` with an `onclick` handler that sets the per-character state (open = true, character = this character). Capture a reference to the form element (e.g. via `bind:this`) so the modal's `onConfirm` can call `formEl.requestSubmit()`. Wire the modal:
  ```svelte
  <ConfirmDialog
    open={removeState.open}
    tone="danger"
    onCancel={() => removeState = { open: false, character: null }}
    onConfirm={() => {
      forms[removeState.character.id].requestSubmit();
      removeState = { open: false, character: null };
    }}
  >
    {#snippet title()}Remove {removeState.character?.name}?{/snippet}
    {#snippet body()}
      This character will be removed from your account. You can add them again
      at any time via add character and performing an EVE login.
    {/snippet}
    {#snippet confirmLabel()}remove character{/snippet}
  </ConfirmDialog>
  ```
- [x] 3.3 Convert the `deleteAccount` form: same pattern. Button becomes `type="button"`, sets `deleteAccountOpen = true`. The modal's `onConfirm` calls the form's `requestSubmit()`. Modal copy:
  ```svelte
  <ConfirmDialog
    open={deleteAccountOpen}
    tone="danger"
    onCancel={() => deleteAccountOpen = false}
    onConfirm={() => { deleteAccountForm.requestSubmit(); deleteAccountOpen = false; }}
  >
    {#snippet title()}Delete account?{/snippet}
    {#snippet body()}
      Your account will be deactivated. To restore it, log back in within
      30 days; after that, your data is permanently removed.
    {/snippet}
    {#snippet confirmLabel()}delete account{/snippet}
  </ConfirmDialog>
  ```
- [x] 3.4 The existing inline error rendering (per-card and page-level) is unchanged. The error path is server-driven and fires AFTER submission, which is AFTER the modal closes — there is no interaction between the modal and the error UI.
- [x] 3.5 No-JS fallback verification (visual inspection of the JSX, not a test): with JS disabled, the per-character `remove` button is `type="button"` and will NOT submit the form. This is a regression vs. the foundation behaviour (no-JS users get no destructive action at all rather than submission-on-first-click). Per design.md decision 8, we accept this — the alternative (a non-JS confirmation page) is out of scope. Document this trade in a one-line comment next to each converted button.

  **Note:** This task supersedes the spec's no-JS-fallback scenario for the v1 implementation. The spec describes the design intent (submission on first click, matching pre-foundation behaviour) which is achievable with a hidden no-JS-only `type="submit"` fallback button if/when a future change wants to close this regression. v1 chooses the simpler implementation.

## 4. Frontend: tests

- [x] 4.1 Add `frontend/src/lib/components/ConfirmDialog.test.ts` (Vitest + `@testing-library/svelte` per the `sveltekit-node` skill's testing rule). Cases:
  - Renders with the title, body, and confirm label snippets from the test setup.
  - Has `role="alertdialog"`, `aria-modal="true"`, and `aria-labelledby` / `aria-describedby` pointing to the title and body elements.
  - Default-focuses the cancel button on open.
  - Calls `onConfirm` when the destructive button is clicked.
  - Calls `onCancel` when the cancel button is clicked.
  - Calls `onCancel` when `Escape` is pressed.
  - Calls `onCancel` when the backdrop is clicked.
  - Does NOT call `onCancel` when the dialog body (outside buttons) is clicked.
  - `Tab` cycles between the two focusable buttons (cancel ↔ destructive).
  - With `prefers-reduced-motion: reduce`, the transition duration is 0 (assert via the value passed to Svelte's `fade`/`scale` transition, mocked via `matchMedia`).
- [x] 4.2 Update `frontend/src/routes/characters/page.server.test.ts`: no change required. The server-side action contract is unchanged; the confirmation is client-side. Add a one-line comment in the test file noting that the modal is tested in `ConfirmDialog.test.ts` and that the server actions remain testable in isolation.
- [x] 4.3 (Optional, if a browser-level harness exists) Add a Playwright-style E2E test for `/characters` that asserts clicking `remove` opens the modal, clicking cancel does not submit, and clicking the destructive button does submit. If no browser harness exists, document a manual verification step in §6.

## 5. Wireframe (author and approve)

- [x] 5.1 Update `frontend/wireframes/characters.html` to reflect the modal. Add two new variants (or annotated sections within the existing file, whichever the file's existing structure supports):
  - **Open state — remove**: render the page with the modal overlaid, showing title `Remove Jita Trader?`, the body text from §3.2, cancel + `remove character` buttons.
  - **Open state — delete account**: render the page with the modal overlaid, showing title `Delete account?`, the body text from §3.3, cancel + `delete account` buttons.
  Both variants SHALL use the same backdrop opacity, dialog max-width, and button spacing as the live component. The closed-state rendering of the page is unchanged.
- [x] 5.2 The user opens the updated wireframe in a browser and signs off. Cosmetic tweaks land in the wireframe AND the live component; spec-affecting changes (e.g. moving the cancel button to the right of the destructive button) require updating `specs/frontend-patterns/spec.md` first.

## 6. Verification

- [ ] 6.1 In a browser at `/characters`, click `remove` on a non-main character. The modal opens; the cancel button has visible focus; the title contains the character's name. Click cancel — modal closes, nothing else changes. Click `remove` again, then click the destructive button — the character is removed (the row disappears and the success/error rendering matches the pre-change behaviour).
- [ ] 6.2 Repeat §6.1 with keyboard only: Tab to the `remove` button, press `Enter`. Modal opens; Tab to the destructive button; press `Enter`. Character is removed. Then: open the modal again, press `Escape`. Modal closes, no submission.
- [ ] 6.3 At `/characters`, click `delete account` under DANGER ZONE. Modal opens with title `Delete account?` and the 30-day body copy. Click cancel — nothing happens. Re-open and click the destructive button — the account is soft-deleted (redirect to `/login` or whichever the foundation behaviour specifies).
- [ ] 6.4 Click outside the dialog body (the backdrop region). Modal closes (cancel path).
- [ ] 6.5 With browser DevTools, simulate `prefers-reduced-motion: reduce` (Rendering tab → Emulate CSS media feature `prefers-reduced-motion`). Re-open the modal. The dialog and backdrop appear instantly with no fade or scale animation.
- [ ] 6.6 With a screen reader (VoiceOver on macOS, NVDA on Windows, or Orca on Linux), open the modal. The screen reader announces the title and body content. Focus is on the cancel button. `Tab` moves to the destructive button.
- [ ] 6.7 Disable JavaScript in the browser. Reload `/characters`. The `remove` and `delete account` buttons are present; clicking them does nothing (per the §3.5 trade documented in design.md decision 8). The character list still renders correctly (server-side rendering is unaffected).
- [ ] 6.8 Open the updated `frontend/wireframes/characters.html` and the live `/characters` side-by-side. The open-state variants in the wireframe SHALL match the live modal's layout, spacing, and colours. Document deliberate deviations as comments in the Svelte component.
