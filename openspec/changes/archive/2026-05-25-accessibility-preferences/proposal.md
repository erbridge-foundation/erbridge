## Status

**Fleshed out** (2026-05-25). Design, specs, and tasks written. Scope and persistence decisions taken during exploration: **full accessibility menu** (not a single toggle) and **backend-backed persistence** via a generic preference substrate, because accessibility is a day-one value. See `design.md` for the decisions and the corrected ground truth (the `--font-base` mechanism this stub assumed had landed never existed; the real text-size knob is `html { font-size }`).

## Why

The `eve-wormhole-mapper-foundation` change ships a `preferences` menu item in the user-menu dropdown as a greyed-out, `aria-disabled` placeholder. The placeholder exists to lock the visual layout in, but it has no destination. Eventually it needs to lead somewhere real, and the obvious thing to put there is an accessibility-preferences page.

A separate, smaller decision is already on the table: a user-controllable **text size**. The font-size tweak that motivated this change (the 14px → 16px → 14px experimentation in the wireframes) made it clear that "what size is comfortable" is a per-user judgement we shouldn't bake into the design tokens. A text-size picker is the smallest possible accessibility preference and the obvious starting point.

We are *not* writing the full design now because designing a settings UI before there are several settings to put in it is a known way to over-engineer. This stub records the direction so a future session can pick it up without rediscovering the requirements.

## What Changes

- A **`/preferences` route** in the SvelteKit frontend, reachable from the user-menu dropdown by turning the existing `aria-disabled` `preferences` placeholder into a real `<a href="/preferences">`. The sibling `settings` placeholder stays disabled (out of scope). The page is reachable anonymously so accessibility can be set without (or before) an account.
- A **generic preference substrate**: a `preferences` JSONB column on `account`, `GET`/`PATCH /api/v1/me/preferences` behind `AuthenticatedAccount`, and a frontend store that is **localStorage-first with backend sync**. `localStorage` is the always-on edge (works anonymously, applied before paint); the backend gives authenticated users cross-device durability. On first login, anonymous setup is pushed up when the server is empty; otherwise the server wins. (Replaces the stub's `--font-base` assumption — that mechanism never existed — and its Stage A/Stage B split, which is collapsed into one localStorage+backend model.)
- **Five accessibility preferences**, each tri-state and defaulting to the relevant OS media query where one exists:
  - `text_size` (`auto`/`small`/`regular`/`large`) — overrides `html { font-size }`; scales the whole `rem`-based UI.
  - `reduce_motion` (`auto`/`on`/`off`) — `auto` follows `prefers-reduced-motion`.
  - `high_contrast` (`auto`/`on`/`off`) — `auto` follows `prefers-contrast: more`.
  - `large_targets` (`off`/`on`) — minimum interactive target sizing.
  - `dyslexia_font` (`off`/`on`) — alternative typeface to JetBrains Mono.
  - (`prefers-color-scheme` is documented N/A — the app is dark-only by design.)
- **No flash-of-unstyled-content**: an inline script in `app.html` applies stored preferences to `<html>` before first paint.
- **Auto-reverting confirmation** for layout-altering changes (`text_size`, `high_contrast`, `large_targets`, `dyslexia_font`): the change previews live and reverts after a countdown (default 10s, a tunable constant) unless the user clicks "Keep", and is persisted only on confirm — so a setting that breaks the page recovers itself with zero user effort. `reduce_motion` is excluded (it can't lock anyone out).
- A **reduce-motion audit** wiring every animation/transition (pulsing `connected` dot, character-grid hover, …) to honour the preference.

## Capabilities

### New Capabilities

- `account-preferences`: the generic, reusable preference substrate — a `preferences` JSONB column on `account`, the `GET`/`PATCH /api/v1/me/preferences` endpoints, the localStorage-first-with-backend-sync model, and login reconciliation (push-local-on-first-login, else server wins). Intended to be reused by `add-internationalisation-support` for `preferences.locale`.
- `accessibility-preferences`: the `/preferences` route and the five accessibility preferences (`text_size`, `reduce_motion`, `high_contrast`, `large_targets`, `dyslexia_font`), their tri-state OS-default behaviour, no-FOUC application before paint, and the auto-reverting confirmation for layout-altering changes.

### Modified Capabilities

- `project-infrastructure` (foundation): the existing "Frontend applies the E-R Bridge design system" requirement's Typography section may need a small amendment to acknowledge that the `--font-base` value is now user-controllable rather than hard-coded to `1rem`.
- `about-page` (depending on what lands first): if the `about` and `preferences` menu items both become real routes, the user-menu ordering decision in `about-page` should be revisited.

## Impact

To be detailed when the change is fleshed out. The minimum dependency footprint is: SvelteKit frontend work (a new route, a new component, a small store), optional backend work (one DB migration + a CRUD endpoint behind `AuthenticatedAccount`), and a small audit of every animation / transition in the codebase to wire up `prefers-reduced-motion`.

## Cross-references

- This stub was created during work on `eve-wormhole-mapper-foundation`, immediately after the tier-1 audit that switched all typography in the wireframes + `frontend/src/app.css` spec from absolute `px` to `rem`. That audit is the necessary precondition for a user-controllable text size — without it, changing `--font-base` would not propagate.
- A breadcrumb comment is left in the foundation's `UserMenu.svelte` next to the `preferences` placeholder pointing at this change.
