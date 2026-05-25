## Why

The shipped `accessibility-preferences` page applies each layout-altering change immediately as a live preview guarded by a 10-second auto-revert countdown (`PreferenceRevertBar`). In use this has two concrete flaws and one conceptual one:

- **Can't batch changes.** Each selection starts (or restarts) its own countdown and replaces the pending patch, so adjusting more than one setting means racing a clock, and only the last patch is committed on "Keep".
- **Reverting to the prior value doesn't stop the clock.** Set `text_size: large`, the countdown starts; set it back to `regular` (its saved value) and there is now nothing to confirm, yet the countdown keeps running.
- **Live preview gives false confidence.** The `/preferences` page is the simplest, most robust surface in the app. A setting that looks fine while previewing there may break a denser page (the map, the character grid) or only bite after a reload — which is exactly the case an on-page countdown covers worst, because the user isn't on the broken page while it ticks.

The fix is to separate *previewing* from *committing*: let the user stage several changes freely as live previews, commit them with an explicit **Apply**, and guarantee recovery through an always-available **Reset to defaults** control rather than a timer.

## What Changes

- Replace per-change preview-and-countdown with an explicit **staging model** on `/preferences`:
  - Changing any control (all five, including `reduce_motion`) enters a **dirty** state: the change previews live on `<html>` but is **not** persisted; **Apply** and **Discard** buttons appear.
  - **Apply** persists the whole batch (localStorage + backend sync) and returns to a clean state. No post-apply countdown.
  - **Discard** reverts the previews to the persisted values. Shown only while dirty.
  - Returning every control to its persisted value returns to the clean state automatically (no buttons, nothing to confirm).
  - Navigating away while dirty **silently auto-discards** the previews, so `<html>` never disagrees with what is persisted.
- Add an **always-available "Reset to defaults"** control that sets all five preferences to their defaults, persists, and applies. This is the cross-page recovery surface: if an applied setting breaks another page, the user returns to the robust `/preferences` page and resets. It replaces the countdown as the lock-out safety guarantee.
- The Apply / Discard / Reset controls keep the contrast- and size-proof styling (fixed `px` sizing, guaranteed contrast) so they remain usable under any applied setting.
- **Remove** the `PreferenceRevertBar` component, the `PREFERENCE_REVERT_SECONDS` constant, and the per-change countdown choreography. `reduce_motion` no longer commits instantly — it stages like the others.
- Add `resetToDefaults()` to the preferences store; keep `preview()`, `commit()`, and `revertToPersisted()`.

Scope is **frontend only**. The `account-preferences` backend substrate and its `GET`/`PATCH /api/v1/me/preferences` endpoints are unchanged.

## Capabilities

### Modified Capabilities

- `accessibility-preferences`: the "Layout-altering preference changes auto-revert unless confirmed" requirement is superseded by a staging/Apply/Discard flow plus an always-available reset-to-defaults recovery control. The set of preferences, their defaults, OS-media-query behaviour, no-FOUC application, and backend sync are unchanged.

## Impact

- **Frontend only.** `frontend/src/routes/preferences/+page.svelte` (staging state machine + Apply/Discard/Reset + navigate-away auto-discard), `frontend/src/lib/preferences/store.svelte.ts` (add `resetToDefaults()`), and the page/store tests are reworked. `PreferenceRevertBar.svelte` and its test are deleted; `PREFERENCE_REVERT_SECONDS` is removed from the schema.
- **No backend change**, no migration, no API change.
- Existing tests that assert the preview/Keep/Revert-now behaviour and `reduce_motion`-commits-instantly must be rewritten.

## Cross-references

- Supersedes part of `accessibility-preferences` (archived `2026-05-25-accessibility-preferences`); the synced capability spec at `openspec/specs/accessibility-preferences/spec.md` is the modification target.
- The contrast/size-proof styling requirement carries over from the deleted `PreferenceRevertBar` to the new Apply/Discard/Reset controls.
