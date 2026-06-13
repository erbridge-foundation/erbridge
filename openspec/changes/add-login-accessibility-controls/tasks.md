## 1. Preset constant

- [ ] 1.1 Define a `MAX_PREFERENCES` constant (the five-key high-accessibility preset: `text_size: 'large'`, `high_contrast: 'on'`, `reduce_motion: 'on'`, `large_targets: 'on'`, `dyslexia_font: 'on'`) as the single source of truth, co-located with the preference schema/defaults in `src/lib/preferences/`. Do NOT include `locale`.

## 2. Localized strings

- [ ] 2.1 Add Paraglide message keys for the new login-page strings: "Maximize accessibility" control label, the on-state/disclosure text ("Applied to this screen…"), the language-selector label, and the post-login guidance ("Adjust anytime via User Menu › Preferences"). Add any needed aria label for the toggle.
- [ ] 2.2 Provide translations for all new keys in `en`, `de`, and `fr` message sets (keep the three locales in sync per project rule).

## 3. Login page controls

- [ ] 3.1 Add the "Maximize accessibility" toggle to `src/routes/login/+page.svelte`. Derive its on/off state from whether the current preference set equals `MAX_PREFERENCES` exactly. On activate, `preferences.commit(MAX_PREFERENCES)`; on deactivate, `preferences.commit` the five keys back to their defaults from `DEFAULT_PREFERENCES`. Leave `locale` untouched.
- [ ] 3.2 Show the on-state disclosure text only when the preset is active.
- [ ] 3.3 Add the language picker (EN / DE / FR) to the login card. Each option calls `preferences.commit({ locale })`; reflect the currently active locale as selected. Confirm the page re-renders in the chosen language via the existing locale→Paraglide bridge.
- [ ] 3.4 Add the post-login guidance as informational text (NOT a link), naming User Menu › Preferences.
- [ ] 3.5 Lay the controls out below the SSO button / disclaimer so the primary login action stays dominant; reuse existing card spacing tokens. Ensure the controls are keyboard-operable and have visible focus.

## 4. Tests

- [ ] 4.1 Vitest: activating the toggle commits all five `MAX_PREFERENCES` keys; deactivating reverts those five keys to their defaults; the toggle's on-state derives correctly from the store state.
- [ ] 4.2 Vitest: the language picker calls `commit({ locale })` and does not alter the accessibility keys.
- [ ] 4.3 Playwright (`/login`): activating "Maximize accessibility" sets the expected `<html>` font-size and `data-*` attributes; selecting a language re-renders the login card strings in that language.

## 5. Verification

Run all three from the `frontend/` directory (no pnpm workspace root):

- [ ] 5.1 `pnpm test` — Vitest unit/component tests pass.
- [ ] 5.2 `pnpm run check` — svelte-check (type checking + paraglide compile) passes with 0 errors / 0 warnings.
- [ ] 5.3 `pnpm run test:e2e` — Playwright e2e tests pass.
