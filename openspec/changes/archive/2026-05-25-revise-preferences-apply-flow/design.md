## Context

`accessibility-preferences` shipped a per-change live-preview-plus-countdown model. `/preferences/+page.svelte` calls `preferences.preview(patch)` on each layout-altering change and shows `PreferenceRevertBar`, a 10-second countdown that either commits ("Keep") or reverts. `reduce_motion` commits immediately via `preferences.commit()`. The store (`lib/preferences/store.svelte.ts`) already exposes `preview()`, `commit()`, and `revertToPersisted()`, plus localStorage-first persistence with backend sync. The schema (`lib/preferences/schema.ts`) defines `PREFERENCE_REVERT_SECONDS` and `LAYOUT_ALTERING_KEYS`.

This change reworks only the page-level interaction and the store's recovery affordance. The substrate (JSONB column, `GET`/`PATCH /api/v1/me/preferences`), the no-FOUC `app.html` bootstrap, the CSS that keys off `<html>` `font-size`/`data-*`, and the localStorage+sync mechanics are all unchanged.

## Goals / Non-Goals

### Goals

- Let the user stage several preference changes and commit them together with an explicit **Apply**.
- Make returning a control to its persisted value cancel the dirty state with no leftover countdown.
- Guarantee `<html>` never disagrees with what is persisted, including on navigate-away.
- Preserve the no-lock-out guarantee via an always-available, contrast/size-proof **Reset to defaults** instead of a timer.

### Non-Goals

- Any backend / API / migration change.
- Changing which preferences exist, their defaults, OS-media-query behaviour, or the no-FOUC bootstrap.
- A cross-page (global) reset affordance — the recovery surface is the `/preferences` page itself, which is robust by construction.

## Decisions

### Decision: A page-local staging model driven by a staged-vs-persisted diff

The page holds a `staged` map (`$state`) initialised from `preferences.current`. A `$derived` diff against `preferences.current` yields `dirty` (any control differs). The UI is a three-state machine:

```
CLEAN (staged == persisted)
  • no Apply/Discard; page at rest
  │ change any control (incl. reduce_motion)
  ▼
DIRTY  — previews live on <html>, NOT persisted; Apply + Discard shown
  ├── set every control back to persisted → auto-CLEAN (derived, no special case)
  ├── [Discard] → revertToPersisted() → CLEAN
  ├── [Apply]   → commit(staged-diff) → CLEAN
  └── navigate away → silently revertToPersisted()

ALWAYS: [Reset to defaults] → resetToDefaults() (persist defaults + apply)
```

`reduce_motion` joins staging — it no longer commits instantly — so all five controls behave uniformly and the page has one consistent interaction.

### Decision: Selecting a control previews live but does not persist

On any control change, the page updates `staged` and calls `preferences.preview(staged)` so `<html>` reflects the staged set immediately. Persistence happens only on Apply. Because the dirty flag is a derived diff (not a flag set on first change), setting a control back to its persisted value naturally returns the diff to empty → CLEAN, with no countdown to stop. This solves the "revert-to-prior should cancel" problem structurally.

### Decision: Navigate-away while dirty silently auto-discards

Previews apply to `<html>`, which outlives the page. If the user leaves while dirty, `<html>` would show a preview that nothing persisted — a lie that a reload would snap back. To prevent this, leaving while dirty reverts the previews:

- **In-app navigation**: SvelteKit `beforeNavigate` → `preferences.revertToPersisted()`.
- **Component teardown**: an `$effect` cleanup (or `onDestroy`) → `revertToPersisted()` as a backstop.
- **Hard reload / tab close**: nothing was persisted, so the reload naturally shows the persisted values; no handler needed (and `beforeunload` can't show our UI anyway).

The discard is **silent** (no confirm prompt). Rationale: leaving without applying is morally "I didn't apply"; the cost of an accidental loss is re-staging, which is cheap, and a prompt on every navigation is heavier than the problem.

### Decision: Reset to defaults is the recovery guarantee, not a timer

The countdown's only unique value was catching a setting that looks fine while previewing but breaks elsewhere or after reload. A timer ticking on the (safe) `/preferences` page covers that case poorly — the user isn't on the broken page while it runs. Instead:

- `/preferences` is the **recovery surface**: it is a simple centred column and, with the Apply/Discard/Reset controls styled to be contrast- and size-proof, it stays usable under any applied setting.
- A **Reset to defaults** control is available in every state. If an applied setting breaks another page, the user navigates back to `/preferences` (always reachable from the user menu) and resets all five preferences to their defaults.

This is a stronger guarantee than the timer: recovery doesn't depend on reacting within N seconds, and it works no matter how long ago the bad setting was applied.

### Decision: Discard and Reset are distinct verbs

- **Discard** — undo the current unsaved staging, back to the last-saved values. Shown only while dirty.
- **Reset to defaults** — set all five preferences to their defaults and persist. Always available; the cross-session recovery action.

They are not the same intent (one returns to *saved*, the other to *defaults*), so both exist.

### Decision: Store gains `resetToDefaults()`; the rest of the API is unchanged

`preview(patch)` (live, unpersisted), `commit(patch)` (persist + sync), and `revertToPersisted()` (re-apply persisted, drop preview) remain. Add `resetToDefaults()`: set the current set to `DEFAULT_PREFERENCES`, apply to `<html>`, persist to localStorage, and sync the defaulting patch to the backend for authenticated users (so a reset propagates cross-device like any other commit).

## Risks / Trade-offs

- **Silent navigate-away discard can surprise a user mid-edit.** Accepted: re-staging is cheap, and previews are visibly unsaved (Apply/Discard on screen). A future enhancement could add an in-app "unapplied changes" prompt without changing the persistence model.
- **Removing the countdown removes a familiar OS-settings affordance.** Accepted: live preview + an always-available reset is a cleaner and arguably safer model for this app, and avoids the timer's false-confidence problem.
- **Reset must itself be usable under a broken setting.** Mitigated by the existing contrast/size-proof styling constraint, now applied to Apply/Discard/Reset.

## Migration

None. Frontend-only behavioural change; persisted preference data and the API are unchanged. Users with existing stored preferences see no difference until they next open `/preferences`.

## Open questions

None outstanding — the interaction model was settled during exploration.
