## Why

A user can load the SvelteKit app and leave the tab open across one or more deploys. The running tab keeps the old client bundle, which can break in subtle ways once a new build is live: lazy-loaded route chunks are content-hashed, so the old bundle requests filenames that no longer exist on the server and navigation 404s. There is currently no signal to the user (or the app) that the running tab is stale.

The `correctly-handle-versions` change gave the frontend a real, git-tag-derived version (`PUBLIC_UI_VERSION` / `APP_VERSION`). That value is currently *passive* — it is only displayed on `/about`. This change makes it *active*: the app detects when the deployed UI version differs from the version the tab is running and offers a non-destructive "reload to update" prompt.

## What Changes

- **Wire SvelteKit's built-in version detection.** Set `kit.version.name` to the git-derived `APP_VERSION` (the same value already injected at build time) and enable polling (`kit.version.pollInterval`). SvelteKit then maintains `updated.current` from `$app/state`, flipping it to `true` when the deployed app version differs from the running one. No hand-rolled polling loop or custom `/version.json` endpoint — the framework already ships this and keys it to a version string we already compute.
- **Add a shared `UpdateBanner` component** under `frontend-patterns` (the capability explicitly scoped to absorb shared frontend primitives). It renders only when `updated.current` is true, shows a localised "a new version is available" message, and offers a reload control. Reloading is `location.reload()` — gated behind an explicit user action so in-progress work (e.g. an unsaved map edit) is never destroyed by a silent reload.
- **Mount the banner once in the root layout** (`+layout.svelte`) so it covers every authenticated route, including `/login`'s sibling routes.
- **i18n.** New paraglide message keys in `en.json` / `de.json` for the banner copy and reload control.

## Capabilities

### Modified Capabilities

- `frontend-patterns`: adds a second shared primitive — a stale-version reload banner driven by SvelteKit's `updated` state — alongside the existing confirmation modal. The capability's purpose statement already anticipates additional shared primitives landing here.

## Impact

- Frontend: `svelte.config.js` gains `kit.version.name` + `kit.version.pollInterval`; new `frontend/src/lib/components/UpdateBanner.svelte`; `+layout.svelte` mounts it; `en.json`/`de.json` gain banner message keys; `app.css`/component styles use existing design tokens.
- Build: `svelte.config.js` must read the same `APP_VERSION` env the vite config already consumes; with no `APP_VERSION` (plain local build) `version.name` falls back to a stable default so dev does not spuriously prompt.
- No backend change. This is a frontend-bundle-staleness signal, NOT backend/API drift — it is keyed to the frontend's own version, not `/api/health`.
- Depends on `correctly-handle-versions` being in place (it provides `APP_VERSION`).
