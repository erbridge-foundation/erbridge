## 1. Version-detection wiring

- [x] 1.1 In `frontend/svelte.config.js`, set `kit.version.name` from `process.env.APP_VERSION` with a fixed fallback (e.g. `"dev"`) when unset, and set `kit.version.pollInterval` to a positive value (suggest `60000`)
- [x] 1.2 Confirm the frontend `Dockerfile` already promotes `APP_VERSION` to `ENV` before `pnpm run build` (added by `correctly-handle-versions`) so `version.name` resolves in the image build; no change expected, just verify

## 2. UpdateBanner component

- [x] 2.1 Add `frontend/src/lib/components/UpdateBanner.svelte`: reads `updated` from `$app/state`, renders only when `updated.current` is `true`, shows the localised message + a reload control that calls `location.reload()`; non-modal (does not block interaction); styled with existing design tokens (mirror the `.layout-error` strip pattern)
- [x] 2.2 Add paraglide message keys for the banner copy + reload label to `frontend/messages/en.json` and `frontend/messages/de.json` (e.g. `update_banner_message`, `update_banner_reload`)

## 3. Mount + wiring

- [x] 3.1 Mount `<UpdateBanner />` once in `frontend/src/routes/+layout.svelte` so it spans all routes (rendered outside the per-route content region)

## 4. Tests

- [x] 4.1 Vitest component test for `UpdateBanner.svelte`: renders nothing when `updated.current` is false; renders the message + reload control when true (mock `$app/state`'s `updated`); activating the control invokes a reload (assert via a mocked `location.reload` / injected handler)
- [x] 4.2 Verify `svelte-check` passes (the `$app/state` `updated` import and any added types type-check)

## 5. Verification

- [x] 5.1 Build the frontend image with two different `APP_VERSION` values; load the first, deploy/serve the second, and confirm the banner appears after the poll interval and a reload fetches the new bundle
- [x] 5.2 Confirm a local `pnpm run build && pnpm run preview` with no `APP_VERSION` does NOT show the banner on a plain rebuild (dev fallback is stable)
- [x] 5.3 Confirm the banner is non-blocking: with it visible, the user can still navigate/interact, and only an explicit reload click reloads
