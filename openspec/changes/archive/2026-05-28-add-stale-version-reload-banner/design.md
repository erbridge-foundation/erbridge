## Context

SvelteKit ships a version-change detection mechanism so apps can notice when a new build has been deployed while a tab is still running an old one. It has two pieces:

- **`kit.version.name`** (build-time): a string identifying this build. SvelteKit writes it into the client manifest. If unset it defaults to a timestamp.
- **`kit.version.pollInterval`** (runtime): how often (ms) the client re-fetches the app's version manifest. When `0` (default) it only checks on navigation; when `> 0` it polls in the background.

The client exposes the result as **`updated`** from `$app/state` (Svelte 5 / SvelteKit ≥ 2.12; the older `$app/stores` `updated` store is deprecated). `updated.current` becomes `true` once the server reports a `version.name` different from the running build's. SvelteKit can also auto-reload on the next navigation when stale (`updated.check()` / the `data-sveltekit-reload` path), but the explicit-prompt UX is preferred here.

`correctly-handle-versions` already computes a git-tag-derived `APP_VERSION` and passes it into the frontend build as an env var (consumed by `vite.config.ts` for `PUBLIC_UI_VERSION`). This change reuses that exact value for `version.name`, so "the version on `/about`" and "the version SvelteKit compares for staleness" are the same string.

## Goals / Non-Goals

**Goals:**
- Detect that the running UI bundle is older than the deployed one, keyed to the git-derived version.
- Offer a non-destructive, user-initiated reload — never silently reload and lose in-progress state.
- Reuse the existing version pipeline; no new server endpoint, no hand-rolled poll.
- One shared component, mounted once, covering the whole app.

**Non-Goals:**
- Detecting *backend*/API version drift (that is `/api/health`'s `commit`/`version`, a different concern; front and back are separate images with independent deploy timing — comparing them would produce false "stale" signals).
- Service-worker / PWA update lifecycle (the app is not a PWA).
- Forcing or auto-reloading without user consent.
- Surfacing *which* version is available or a changelog — the banner only says "newer exists, reload".

## Decisions

### D1: Use SvelteKit's built-in `updated`, not a custom poller
SvelteKit already implements the manifest fetch + compare + cache-busting and exposes `updated.current`. Reimplementing it (a `setInterval` fetching a custom `/version.json`) would duplicate framework behaviour and drift. The only configuration needed is `version.name` (what to compare) and `pollInterval` (how often).

### D2: `version.name = APP_VERSION` (same source as `PUBLIC_UI_VERSION`)
`svelte.config.js` reads `process.env.APP_VERSION` and uses it for `kit.version.name`. The frontend `Dockerfile` already promotes `APP_VERSION` to an `ENV` before `pnpm run build` (from `correctly-handle-versions`), so it is present at config-evaluation time. Fallback when unset: a fixed string (e.g. `"dev"`) rather than SvelteKit's default timestamp, so a local `pnpm run build && pnpm run preview` does not flip `updated.current` on every rebuild and spam the banner in dev. A fixed dev value means develop builds only differ when `APP_VERSION` actually changes — which is the intended behaviour.

### D3: Explicit reload, never silent
`updated.current === true` renders the banner; the user clicks Reload → `location.reload()`. We do NOT use SvelteKit's auto-reload-on-navigation. Rationale: the app holds unsaved client state (map edits, in-flight forms); a navigation- or timer-triggered reload could discard it. The banner is persistent but dismissible-by-acting, not modal — it must not block interaction so the user can finish their work first.

### D4: Mount once in the root layout
The banner lives in `+layout.svelte` so it spans all routes. It is rendered outside the per-route content region. It does not render on routes where it would be meaningless (none currently — even `/login` benefits), so no route gating beyond "render when `updated.current`".

### D5: Component lives in `frontend-patterns`
Per the `sveltekit-node` skill, shared components live in `frontend/src/lib/components/`. The capability `frontend-patterns` is explicitly scoped for shared primitives. Canonical path: `frontend/src/lib/components/UpdateBanner.svelte`.

## Risks / Trade-offs

- **R1 — `pollInterval` too aggressive wastes requests; too lax delays the prompt.** A modest interval (e.g. 60s) balances responsiveness against noise. The manifest is tiny. Documented as a single config value, easy to tune.
- **R2 — `version.name` must be stable per build.** If it accidentally became a per-process random value, every poll would report "stale". Mitigation: it is the deterministic git-derived `APP_VERSION`, and the dev fallback is a fixed string (D2).
- **R3 — banner could obscure UI.** Mitigation: it is a thin, non-modal strip using existing layout-banner styling (mirrors the existing `.layout-error` strip), not an overlay.
- **R4 — depends on `correctly-handle-versions`.** If that change's `APP_VERSION` wiring regressed, `version.name` would fall back to the dev default and never prompt. Acceptable: the feature degrades to "no prompt", never to a broken app.

## Open Questions

- Poll interval value (60s suggested) — confirm during implementation; it is a one-line tunable.
