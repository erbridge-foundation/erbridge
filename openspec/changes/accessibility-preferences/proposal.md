## Status

**Stub.** This proposal captures intent — the specs, design, and tasks are not written yet. Run `/opsx:explore accessibility-preferences` to flesh it out when the time comes. Until then, this stub lives in `openspec list` as a forward-looking placeholder so the work is not forgotten.

## Why

The `eve-wormhole-mapper-foundation` change ships a `preferences` menu item in the user-menu dropdown as a greyed-out, `aria-disabled` placeholder. The placeholder exists to lock the visual layout in, but it has no destination. Eventually it needs to lead somewhere real, and the obvious thing to put there is an accessibility-preferences page.

A separate, smaller decision is already on the table: a user-controllable **text size**. The font-size tweak that motivated this change (the 14px → 16px → 14px experimentation in the wireframes) made it clear that "what size is comfortable" is a per-user judgement we shouldn't bake into the design tokens. A text-size picker is the smallest possible accessibility preference and the obvious starting point.

We are *not* writing the full design now because designing a settings UI before there are several settings to put in it is a known way to over-engineer. This stub records the direction so a future session can pick it up without rediscovering the requirements.

## What Changes

The eventual change will include some combination of the following. The specifics are deferred:

- A **`/preferences` route** in the SvelteKit frontend, reachable from the user-menu dropdown by removing the `aria-disabled` attribute from the existing `preferences` item and pointing its `href` at `/preferences`.
- A **text-size preference** with at least three steps (e.g. `small` / `regular` / `large`). The selected value sets a CSS custom property `--font-base` on `<html>` (or equivalent) that every typography rule already keys off. The `rem`-everywhere groundwork from foundation §4.3 + design.md §10 (landed alongside this stub's creation) is what makes this trivial to implement.
- Per-user **persistence**. Two-stage rollout is recommended:
  - **Stage A** (smaller change): persist to `localStorage`, anonymous and per-browser. No DB migration. No backend involvement.
  - **Stage B** (this change, or a follow-up): introduce an `account_preferences` table (or JSONB column on `account`) so the choice survives across devices for authenticated users. The `localStorage` value is migrated to the server when an authenticated user changes a setting.
- Honour OS-level accessibility media queries by default:
  - `prefers-reduced-motion: reduce` — disable the pulsing `connected` dot animation, character grid hover transitions, etc.
  - `prefers-color-scheme` — N/A for this app (it is dark-only by design), but worth a documented decision.
  - `prefers-contrast: more` — a higher-contrast colour-token override.
- Additional preferences to consider (out of scope to specify here, but useful to enumerate so the eventual UI is designed for more than just one toggle):
  - Reduce motion (toggle, defaulting to the OS preference).
  - High-contrast colour palette (toggle, defaulting to the OS preference).
  - Larger interactive targets (toggle).
  - Dyslexia-friendly typeface as an alternative to JetBrains Mono (toggle).

## Capabilities

### New Capabilities

- `accessibility-preferences`: defines the `/preferences` route, the set of preferences, their default values, how they are persisted (localStorage vs. backend), and how they interact with OS-level media queries.

### Modified Capabilities

- `project-infrastructure` (foundation): the existing "Frontend applies the E-R Bridge design system" requirement's Typography section may need a small amendment to acknowledge that the `--font-base` value is now user-controllable rather than hard-coded to `1rem`.
- `about-page` (depending on what lands first): if the `about` and `preferences` menu items both become real routes, the user-menu ordering decision in `about-page` should be revisited.

## Impact

To be detailed when the change is fleshed out. The minimum dependency footprint is: SvelteKit frontend work (a new route, a new component, a small store), optional backend work (one DB migration + a CRUD endpoint behind `AuthenticatedAccount`), and a small audit of every animation / transition in the codebase to wire up `prefers-reduced-motion`.

## Cross-references

- This stub was created during work on `eve-wormhole-mapper-foundation`, immediately after the tier-1 audit that switched all typography in the wireframes + `frontend/src/app.css` spec from absolute `px` to `rem`. That audit is the necessary precondition for a user-controllable text size — without it, changing `--font-base` would not propagate.
- A breadcrumb comment is left in the foundation's `UserMenu.svelte` next to the `preferences` placeholder pointing at this change.
