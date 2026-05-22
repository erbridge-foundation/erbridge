## Status

**Stub.** This proposal captures intent — the specs, design, and tasks are not written yet. Run `/opsx:explore confirm-destructive-actions` to flesh it out when the time comes. Until then, this stub lives in `openspec list` as a forward-looking placeholder so the work is not forgotten.

## Why

The `eve-wormhole-mapper-foundation` change ships three destructive actions on the `/characters` page — `remove` (deletes a non-main character), `delete account` (soft-deletes the account), and indirectly `re-auth` (replaces stored tokens) — each wired as a single-click form submission with no confirmation step. The approved wireframes (`characters.html`) and design.md §14 (form-action error treatment) do not specify a confirmation pattern; the actions fire as soon as the button is pressed.

This was acceptable for the foundation change because the wireframes were already signed off and shipping unblocked everything downstream, but a single-click `delete account` button is a UX hazard. A user who clicks `delete account` by reflex has no recovery path inside the UI — they must wait, re-log in, and notice the reactivation behaviour (per spec §7.22) to undo the action. Even `remove character` is unforgiving: the row is hard-deleted, the encrypted refresh token is gone, and re-adding the character requires a fresh SSO flow.

A confirmation step protects against accidental clicks without changing the underlying contract (the backend still returns the same status codes and error envelope on success/failure).

We are *not* writing the full design now because the confirm-dialog pattern has several reasonable shapes (native `confirm()`, inline two-step button, modal dialog) and choosing the right one belongs in a focused conversation rather than buried in the foundation change. This stub records the gap so a future session picks it up before users encounter it in production.

## What Changes

The eventual change will introduce a confirmation step in front of every destructive action on `/characters`. The specifics are deferred, but the design space includes:

- **Action coverage.** At minimum: `remove character` and `delete account`. Optionally: `set main` (low-risk, probably skip), `re-auth` (re-runs SSO; harmless but redirects away, so the user shouldn't be surprised). Worth deciding whether the confirmation also applies to API-key deletion (when that surface exists) and to future destructive actions added by later changes.
- **UI shape.** Three plausible patterns:
  - **Native `window.confirm()`** — zero design work, accessible by default, but disruptive and ugly. Reasonable as a stopgap, not as a final answer.
  - **Inline two-step button** — `remove` → button text flips to `confirm remove?` for a few seconds; second click commits. Lightweight, no overlay, but easy to miss and hard to make accessible.
  - **Modal dialog** — explicit dialog with the destructive action name, a brief consequence statement ("This will permanently delete the character's stored tokens and remove it from your account."), and `cancel` / `confirm` buttons. Most discoverable and accessible; needs new component work.
- **Wireframe update.** The selected pattern SHALL be reflected in `wireframes/characters.html` (or, post-archival, `frontend/wireframes/characters.html`). The pre-archival move described in foundation task 7.29 means the wireframe lives in `frontend/wireframes/` by the time this change runs — confirm the location before editing.
- **Differentiation of severity.** `delete account` is irreversible from the UI's perspective (the only undo is re-running SSO to reactivate). `remove character` is less destructive but still permanent. The confirmation copy SHALL distinguish them — a single shared dialog with placeholder copy is the cheap path; per-action copy is the better one.
- **Accessibility.** Whatever shape is picked SHALL trap focus (modal), be dismissible via `Escape`, announce itself to assistive tech (`role="alertdialog"`), and have a clearly-named cancel button.
- **No backend changes** are expected. The confirmation is purely client-side; the existing form actions and 4xx error envelope continue to handle the actual destruction.

## Impact

- **Affected specs:** likely `account-management` (only to note the UI confirmation requirement; the API contract stays unchanged) and the `eve-wormhole-mapper-foundation` design.md §14 (which currently describes error treatment without mentioning confirmation).
- **Affected code:** `frontend/src/routes/characters/+page.svelte` (button → form submission wired through a confirmation step), one new shared component if the modal path is chosen (e.g. `frontend/src/lib/components/ConfirmDialog.svelte`).
- **Risk:** very low. The change is additive and reversible; nothing depends on the absence of a confirmation step.
