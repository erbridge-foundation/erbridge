## Why

The frontend already handles `prefers-reduced-motion` and `prefers-contrast` well (tri-state `auto`/`on`/`off` preferences keyed off OS media queries). Two OS/browser-driven rendering cases remain unhandled, and one of them can actively break the UI:

- **Windows High Contrast Mode (`forced-colors: active`)** — when active, the OS *replaces* our palette with the user's chosen system colors. Our `high_contrast` preference does nothing here (the browser overrides it). Worse, two patterns we use everywhere break under it: (1) focus is indicated by `outline: none; border-color: var(--sky)` across ~10 components — `border-color` is OS-overridden in forced-colors mode, so a focused input becomes indistinguishable from an unfocused one, stranding keyboard users; (2) status dots/chips carry meaning **by colour alone** (connected=emerald, error=red, warning=amber), which forced-colors flattens, destroying the signal. This is a real-user (low-vision) case, not a nice-to-have.
- **Native control rendering** — the app is dark-only (a deliberate stance: this is an EVE tool), but we never declared `color-scheme: dark`, so the browser renders native controls (date pickers, scrollbars, `<select>` dropdowns, spinners) in their default light scheme, mismatched against our dark surfaces. Native form controls also don't pick up our `--sky` accent.

## What Changes

All changes live in `frontend/src/app.css` (the existing accessibility-preferences home). Dark-only is a hard assumption that keeps this small.

- **`color-scheme: dark`** — declare it on `:root` so the browser renders native controls, scrollbars, and form-control internals in dark mode to match our surfaces. One declaration; no light theme implied.
- **`accent-color: var(--sky)`** — declare it on `:root` so native checkboxes, radios, and range inputs adopt the theme accent.
- **`@media (forced-colors: active)` block** — a single centralized block (matching how `prefers-reduced-motion`/`prefers-contrast` are handled centrally rather than per-component):
  - Restore an **outline-based focus indicator** on `:focus-visible` using a system colour keyword, so the everywhere `outline: none; border-color: var(--sky)` pattern still shows visible keyboard focus when the OS overrides `border-color`.
  - Ensure structural **borders** use system colour keywords (`CanvasText`/`ButtonBorder`) where a border relying on a subtle `--space-*` colour would otherwise vanish.
  - Preserve **meaning-by-colour** signals (status dots/chips: connected/error/warning) by applying `forced-color-adjust: none` to the specific elements where colour *is* the information, so the semantic colour survives the OS flatten.
- **`prefers-color-scheme` is NOT a light theme.** Dark-only is deliberate; the `color-scheme: dark` declaration is the entirety of our colour-scheme handling. Recorded in design as an explicit non-goal.

## Capabilities

### Modified Capabilities
- `accessibility-preferences`: Adds OS/browser-driven rendering behaviour that is *not* a user-facing preference toggle — `color-scheme: dark`, the native-control accent, and `forced-colors: active` support (focus, borders, meaning-by-colour). These extend the same `app.css` surface the existing preferences live in but apply unconditionally (no new preference key, no new control).

## Impact

- **Code:** `frontend/src/app.css` only (the `:root` declarations and one new `@media (forced-colors: active)` block). A small number of meaning-by-colour components (status dots/chips in the layout/nav, `/characters`, `/account`, admin grids) may need a class/selector the forced-colors block can target with `forced-color-adjust: none` — done by selector from `app.css` where possible to keep the logic centralized.
- **Dependencies:** none.
- **Tests:** `forced-colors` and `color-scheme` cannot be asserted in jsdom/Vitest (no media-query emulation for these). Coverage is: (1) a static assertion that `app.css` declares `color-scheme: dark` and contains a `forced-colors: active` block (guards against regression/removal); (2) a Playwright e2e spec using `page.emulateMedia({ forcedColors: 'active' })` (Chromium) that asserts the actual outcomes — keyboard focus produces a visible `outline`, and a colour-encoded signal element has `forced-color-adjust: none`; (3) manual verification on Windows High Contrast (documented in tasks) as the final visual check; (4) the existing reduce-motion/contrast tests are unaffected. Vitest, `svelte-check`, and Playwright must all pass per the project rule.
- **Behaviour:** no change for users not in forced-colors mode except that native controls now render dark (an improvement). No new user-facing preference. No schema/backend change. No `prefers-reduced-data`/`inverted-colors`/light-theme work (out of scope).
