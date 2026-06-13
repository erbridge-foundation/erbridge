## Why

The app already has a complete accessibility/locale preference system (text size, contrast, motion, target size, dyslexia font, language) that applies to every route — including the login page — via the no-FOUC bootstrap and the localStorage-first store. But a user who needs those adjustments has no way to reach them *from the login screen*: `/preferences` is only discoverable post-login via the user menu, and a first-time or low-vision visitor faces an un-adjustable login card. We want to surface the existing system at the one moment a struggling user most needs it — before they sign in — without rebuilding any of it.

## What Changes

- Add a **"Maximize accessibility" toggle** to the login card. Turning it on applies a fixed high-accessibility preset through the existing preference store (`commit`): `text_size: large`, `high_contrast: on`, `reduce_motion: on`, `large_targets: on`, `dyslexia_font: on`. It takes effect instantly (the store applies to `<html>` and the login card visibly transforms in place). The toggle is reversible: turning it off reverts those keys to their defaults. `locale` is left untouched (language is its own control).
- Add a **language picker** (EN / DE / FR) to the login card. Selecting a language calls `commit({ locale })`; the page re-renders in the chosen language using the already-translated login strings.
- Add **guidance, not a link**: a short hint that these settings can be adjusted later via *User Menu › Preferences*. It is informational text (no anchor) — the destination exists only after login.
- Persistence rides the existing flow with **no change**: the preset/locale write to localStorage immediately. A first-time user's choices are promoted to their account on first sign-in by the store's existing `reconcile()`; an existing user's saved account preferences correctly win on sign-in (server-wins precedence is intentional). The login-page controls are therefore framed as "applied to this screen," not "saved to your account."
- Add the new user-facing strings (toggle label + state, language picker label, the Preferences guidance) to all three locales (en/de/fr).

This is **frontend-only**: no backend, schema, migration, store, or auth-flow change. It composes existing primitives (`preferences.commit`, the locale→Paraglide bridge, the existing translated login messages) onto the login card.

## Capabilities

### New Capabilities
<!-- none — this reuses the existing accessibility-preferences capability -->

### Modified Capabilities
- `accessibility-preferences`: Adds a requirement that the login page exposes a one-action high-accessibility preset and a language selector that drive the existing preference substrate, plus the requirement that these surface as "apply now for this screen" with post-login adjustment guidance (no new store, endpoint, preference key, or persistence path).

## Impact

- **Code:** `frontend/` only.
  - `src/routes/login/+page.svelte` — add the toggle, language picker, and guidance text; wire them to `preferences.commit` and the existing store.
  - A small `MAX_PREFERENCES` preset constant (alongside `src/lib/preferences/schema.ts` or local to the login route) defining the five preset values.
  - Paraglide messages: ~4–5 new keys across `messages/` and the en/de/fr message sets.
- **Dependencies:** none.
- **Persistence / auth:** unchanged. Reuses `commit` (writes localStorage; backend sync silently no-ops without a session) and the existing `reconcile()` first-login promotion. The OAuth `state` / in-flight record is **not** touched.
- **Tests (per project rule, run from `frontend/`):**
  - Vitest — the preset toggle applies all five keys via the store and reverts them to defaults when toggled off; the language picker calls `commit({ locale })`.
  - `pnpm run check` — svelte-check + paraglide compile (keeps the new keys in sync).
  - Playwright — on `/login`: toggling "Maximize accessibility" sets the expected `<html>` attributes/font-size; selecting a language re-renders the login card in that language.
- **Interaction note:** the in-flight `add-forced-colors-support` change touches the same `accessibility-preferences` capability and `app.css` but is orthogonal (OS-driven rendering, no controls) — no overlap with these login-page controls.
- **Behaviour for existing users:** a login-page tweak is overridden by their saved account preferences on sign-in (intended); the guidance text avoids promising otherwise.
