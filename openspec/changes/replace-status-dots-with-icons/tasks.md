## 1. StatusIcon component

- [x] 1.1 Create `frontend/src/lib/components/StatusIcon.svelte` with props `level: 'ok' | 'warning' | 'error'` and optional `tooltip?: string`.
- [x] 1.2 Render one inline `<svg>` per level using `currentColor`: `ok` = check inside a circle, `error` = cross inside a circle, `warning` = bang inside a triangle, with the circle diameter equal to the triangle bounding box so all three share one optical bounding box.
- [x] 1.3 Map each level to its design-token colour (`--emerald` / `--amber` / `--red`) via `currentColor` on the wrapper, with no pulse animation.
- [x] 1.4 Implement the two tooltip modes: no `tooltip` → `aria-hidden`, non-focusable, decorative; with `tooltip` → focusable and tooltip associated accessibly (e.g. `aria-describedby`), never a bare `title`.

## 2. Component tests

- [x] 2.1 Create `frontend/src/lib/components/StatusIcon.test.ts` asserting the correct distinct glyph renders for each level.
- [x] 2.2 Assert the decorative mode (no tooltip → `aria-hidden`, not focusable) and the tooltip mode (focusable + accessibly associated, not just `title`).

## 3. Convert call sites

- [x] 3.1 `frontend/src/lib/components/GlobalNav.svelte` — replace the connection dot with `StatusIcon`: connected → `ok`, disconnected → `error`; keep the existing "Connected"/"Disconnected" text; remove the old `.dot` CSS and the pulse keyframes.
- [x] 3.2 `frontend/src/routes/admin/characters/+page.svelte` — replace the token-status dots and the issues-cell dots with `StatusIcon` (active → `ok`, owner_mismatch → `warning`, expired → `error`); keep the existing text; delete the now-dead `.dot` / status CSS.
- [x] 3.3 `frontend/src/routes/characters/+page.svelte` — replace the user-facing token-status dot with `StatusIcon` (same mapping); delete the `.token-status` dot CSS that was copy-pasted from the admin page.

## 4. Docs

- [x] 4.1 Update `openspec/AGENTS.md` component tree to add `StatusIcon`.

## 5. Verification (run from `frontend/`)

- [x] 5.1 `pnpm test` — Vitest unit/component tests pass (including the new StatusIcon tests).
- [x] 5.2 `pnpm run check` — svelte-check (type check + paraglide compile) passes 0 errors / 0 warnings.
- [x] 5.3 `pnpm run test:e2e` — Playwright e2e tests pass.
