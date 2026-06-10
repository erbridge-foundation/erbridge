## Context

The accessibility-preferences capability already implements `prefers-reduced-motion` and `prefers-contrast` as tri-state preferences (`auto` follows the OS via `@media`; `on`/`off` override via `data-*` attributes on `<html>`). All of it lives in `frontend/src/app.css`, with a no-FOUC bootstrap in `app.html` that applies stored overrides before first paint.

Two OS/browser-driven cases are unhandled and are *not* preference toggles — they apply unconditionally:

- `forced-colors: active` (Windows High Contrast / Contrast Themes): the OS replaces the page palette with system colours and ignores author colours. CSS custom properties that carry colour are overridden; only structural CSS and explicit system-colour keywords / `forced-color-adjust` survive.
- Native control rendering: without `color-scheme`, the UA renders form controls, scrollbars, and pickers in the light default, mismatched against our dark surfaces.

Two existing patterns interact badly with forced-colors and motivate the focus and chip requirements:

1. **Focus pattern.** ~10 components use `input:focus { outline: none; border-color: var(--sky); }`. Under `forced-colors: active` the UA forces border colours toward system keywords, so the `--sky` focus border collapses into the resting border and keyboard focus becomes invisible.
2. **Meaning-by-colour.** Status dots and chips (connected=`--emerald`, error=`--red`, warning=`--amber`) encode state purely in colour. forced-colors flattens these to system colours, destroying the distinction unless the element opts out with `forced-color-adjust: none`.

## Goals / Non-Goals

**Goals:**
- Make the app usable and legible under Windows High Contrast: visible keyboard focus, surviving structural borders, and preserved status-signal colours.
- Render native controls in dark to match the app (`color-scheme: dark`) and theme native form controls to the accent (`accent-color`).
- Keep the handling **centralized** in `app.css`, mirroring how reduce-motion/contrast are handled, rather than scattering per-component overrides.

**Non-Goals:**
- A light theme / `prefers-color-scheme: light` support. Dark-only is deliberate (EVE tool). `color-scheme: dark` is a browser hint, not a theme switch.
- A new user-facing preference or control. These behaviours apply unconditionally; there is no `auto`/`on`/`off` toggle and no new preference key, schema field, or `data-*` attribute.
- `prefers-reduced-transparency`, `inverted-colors`, `prefers-reduced-data` — out of scope (low value / negligible support for this app; the app uses no backdrop-blur/translucency, so reduced-transparency is moot).
- Any backend/schema change.

## Decisions

### Decision: Modify `accessibility-preferences`, do not create a new capability

This work extends the same `app.css` surface and the same OS-media-query philosophy the capability already owns. It is *not* a discrete user-facing surface (cf. `maps-acls-ui`, which got its own capability because it added routes/controls). There is no new control or preference key. So it is a MODIFIED capability — new requirements added to `accessibility-preferences` — not a new one.

### Decision: forced-colors handling is unconditional, not a tri-state preference

Unlike `high_contrast` (which is a user choice with `auto`/`on`/`off`), `forced-colors: active` is an OS *mode* the user has already chosen at the OS level; the page must simply respect it. Adding a toggle would be meaningless (you cannot opt out of the OS overriding your colours from CSS). So no new preference key, no `data-*` attribute, no bootstrap change, no control on `/preferences`.

### Decision: Restore focus with an `outline` on `:focus-visible`, not by reworking every component

The existing `outline: none; border-color: var(--sky)` pattern is fine in normal mode. Rather than rewrite ~10 components, the forced-colors block adds, for that mode only, a real `outline` on `:focus-visible` (using a system colour). `:focus-visible` (not `:focus`) keeps it keyboard-scoped and consistent with the rest of the app's focus styling. This is intrinsic to doing forced-colors correctly — not a separate focus audit.
- **Alternative considered:** drop `outline: none` globally and rely on the UA outline everywhere. Rejected: changes normal-mode appearance across the whole app for a forced-colors-only need.

### Decision: Preserve status-signal colour with `forced-color-adjust: none`, scoped narrowly

For elements where colour *is* the information (status dots/chips: connected/error/warning), apply `forced-color-adjust: none` so the semantic `--emerald`/`--red`/`--amber` survives the OS flatten. Applied **only** to those specific signal elements — everything else should honour the user's forced palette.
- **Alternative considered:** add a non-colour cue (icon/text) to every status signal instead of preserving colour. More robust (helps users who can't distinguish the forced palette) but larger and touches many components' markup; deferred as a possible follow-up. Preserving colour is the minimal correct step and keeps the change `app.css`-centric. Recorded here so the trade-off is explicit.

### Decision: `color-scheme: dark` and `accent-color` go on `:root`, unconditionally

Both are single declarations on `:root`. `color-scheme: dark` tells the UA to render native controls/scrollbars dark; `accent-color: var(--sky)` themes native checkboxes/radios/range. Neither implies a light theme nor needs the high-contrast/forced-colors palette (under `forced-colors: active` the UA controls native-control colour regardless, so these are simply inert there — no conflict).

## Risks / Trade-offs

- **Verification is partly manual.** jsdom/Vitest cannot emulate `forced-colors`/`color-scheme`. We guard against *removal* with a static `app.css` content assertion, but the *visual outcome* (focus visible, chips legible) is verified manually on Windows High Contrast and recorded in tasks. This is acceptable for a CSS-only, additive change with no normal-mode behaviour change.
- **Scoped `forced-color-adjust: none` could be forgotten on future status signals.** Mitigated by a contributor note in `app.css` (next to the forced-colors block) stating the rule: any new meaning-by-colour element must opt out there.
- **`color-scheme: dark` very slightly changes native-control appearance for all users** (date pickers/scrollbars now dark). This is the intended improvement, not a regression.
