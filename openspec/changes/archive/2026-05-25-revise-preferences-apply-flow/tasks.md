## 1. Store: add reset, keep the rest (per `sveltekit-node` skill)

- [x] 1.1 Add `resetToDefaults()` to `lib/preferences/store.svelte.ts`: set current set to `DEFAULT_PREFERENCES`, apply to `<html>`, persist to localStorage, and sync the defaulting patch to the backend (authenticated users). Keep `preview()`, `commit()`, `revertToPersisted()`.
- [x] 1.2 Store unit tests: `resetToDefaults()` sets all keys to defaults, persists, and syncs; existing preview/commit/revert tests still pass.

## 2. Schema cleanup

- [x] 2.1 Remove `PREFERENCE_REVERT_SECONDS` from `lib/preferences/schema.ts` (and any now-unused references). `LAYOUT_ALTERING_KEYS` is no longer needed to gate a countdown — keep it only if still referenced; otherwise remove.
- [x] 2.2 Update `schema.test.ts` for any removed exports.

## 3. Remove the revert bar

- [x] 3.1 Delete `lib/components/PreferenceRevertBar.svelte` and `lib/components/PreferenceRevertBar.test.ts`.

## 4. Rework the /preferences page

- [x] 4.1 `routes/preferences/+page.svelte`: replace the per-change preview+countdown with a staging model — a `staged` `$state` initialised from `preferences.current`, a `$derived` dirty diff, and `preferences.preview(staged)` on each control change (all five controls, incl. `reduce_motion`).
- [x] 4.2 Add **Apply** (commit the staged diff → clean) and **Discard** (`revertToPersisted()` → clean) controls, shown only while dirty; returning all controls to persisted values auto-cleans via the derived diff.
- [x] 4.3 Add an always-available **Reset to defaults** control wired to `resetToDefaults()`.
- [x] 4.4 Auto-discard on leave: `beforeNavigate` → `revertToPersisted()` for in-app nav, plus an `$effect` cleanup / `onDestroy` backstop on unmount.
- [x] 4.5 Style Apply / Discard / Reset to be contrast- and size-proof (fixed `px` sizing, guaranteed contrast), carrying over the constraint from the deleted revert bar.

## 5. Tests + verification

- [x] 5.1 Rewrite `routes/preferences/page.svelte.test.ts`: drop the preview/Keep/Revert-now and reduce_motion-commits-instantly assertions; add staging (preview-not-persisted), Apply-commits-batch, Discard-reverts, return-to-prior-cleans, and Reset-to-defaults coverage.
- [x] 5.2 Run `svelte-check`, `vitest run`, and `pnpm build` — all green.
