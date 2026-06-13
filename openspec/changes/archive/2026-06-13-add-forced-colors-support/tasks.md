## 1. Native control rendering (`app.css`)

- [x] 1.1 Add `color-scheme: dark;` to the `:root` block in `frontend/src/app.css`, with a comment noting the app is dark-only (this is a UA rendering hint, not a light theme) and pointing to the accessibility-preferences spec.
- [x] 1.2 Add `accent-color: var(--sky);` to `:root` so native checkboxes/radios/range inputs use the theme accent.

## 2. Forced-colors support (`app.css`)

- [x] 2.1 Add a single `@media (forced-colors: active)` block to `app.css`, placed near the existing accessibility rules. Lead it with a CONTRIBUTOR NOTE (mirroring the reduce-motion note) explaining that author colours are OS-overridden here, that focus/borders must use system-colour keywords, and that any new colour-encoded signal element must be opted out with `forced-color-adjust: none` in this block.
- [x] 2.2 In the block, restore an outline-based keyboard-focus indicator on `:focus-visible` using a system colour keyword, so the everywhere `outline: none; border-color: var(--sky)` pattern still shows visible focus.
- [x] 2.3 In the block, give structural borders that rely on subtle `--space-*` colours a system-colour keyword (`ButtonBorder`/`CanvasText`) so they do not vanish.

## 3. Preserve colour-encoded status signals

- [x] 3.1 Identify the meaning-by-colour signal elements (the connected status dot in the layout/nav; status/token chips in `/characters`, `/account`, and the admin character/blocks grids). Confirm each has a stable selector/class the forced-colors block can target; add a minimal class only where one is missing (no markup restructuring).
- [x] 3.2 In the `forced-colors: active` block, apply `forced-color-adjust: none` to those signal elements (and only those) so `--emerald`/`--red`/`--amber` survive the OS flatten.

## 4. Regression guard test

- [x] 4.1 Add a Vitest test that reads `frontend/src/app.css` and asserts it contains `color-scheme: dark`, `accent-color: var(--sky)`, and an `@media (forced-colors: active)` block (jsdom cannot emulate these modes, so this guards against accidental removal). Follow existing co-located test conventions.
- [x] 4.2 Add a Playwright e2e spec under `frontend/tests/e2e/` that runs with `page.emulateMedia({ forcedColors: 'active' })` (Chromium-supported) and asserts the behaviour jsdom cannot: keyboard-focusing an input that uses the `outline: none; border-color: var(--sky)` pattern yields a non-`none` computed `outline-style` (focus visible), and a colour-encoded status element (e.g. the connected dot / a status chip) has computed `forced-color-adjust: none` (signal preserved). Drive a page that already shows those elements (e.g. `/preferences` for focusable controls; an authed view for the status signal, or the connected dot in the nav).

## 5. Verification

- [x] 5.1 `pnpm --filter frontend test` — Vitest unit/component tests pass.
- [x] 5.2 `pnpm --filter frontend run check` — svelte-check (type checking + paraglide compile) passes with no errors.
- [x] 5.3 `pnpm --filter frontend run test:e2e` — Playwright e2e tests pass.
- [x] 5.4 Manual verification on Windows High Contrast / Contrast Themes (documented outcome, since it is not automatable): keyboard focus is visible on inputs/buttons; structural borders remain visible; the connected dot and status/error/warning chips keep their semantic colour. Spot-check native controls (scrollbar, `<select>`, date picker) render dark with `color-scheme: dark`.
  - Verified 2026-06-13 on real Windows Contrast Themes (Edge/Chromium): keyboard focus shows the restored outline on the `/characters` search input; card/table/input borders remain visible across `/characters` and `/admin/blocks`; connected nav dot + "token active" dot stay green (other token states share the same `.token-status` `forced-color-adjust: none` rule). With the contrast theme off, the `/admin/audit` Event-type `<select>` menu and its scrollbar render dark (`color-scheme: dark`).

## 6. OpenSpec hygiene

- [x] 6.1 Run `openspec validate add-forced-colors-support --strict` and resolve any issues.
- [x] 6.2 Update memory (`project-frontend-status.md`, and/or the accessibility note) to record forced-colors + native-control theming once implemented.
