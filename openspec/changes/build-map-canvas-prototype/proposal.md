## Why

The static wireframe `frontend/static/wireframes/map_canvas.html` (~2920 lines) nails the
*visual* language of the map but cannot prove the *interaction* model — drag / pan / zoom /
collision / one-shot layout — which only a real svelte-flow build reveals. We need a living,
interactive successor to the wireframe, dressed in the real app design tokens, to (a) decide
whether `@xyflow/svelte` is the right canvas for this interaction model and (b) use the
canvas as a requirements-gathering instrument that surfaces what state the (separate, later)
backend chain-map model must expose.

The exploration that produced this is recorded in
`openspec/changes/explore-map-canvas-prototype/design.md` (canvas forks RESOLVED; backend
model forks deliberately OUT of scope here).

## What Changes

- Add `@xyflow/svelte` as the **only** new frontend dependency.
- Add `$lib/components/MapCanvas.svelte` — a real, reusable canvas component that consumes a
  **position-less** combined graph (`{ systems, connections, roots-per-tab }`) and renders it
  through svelte-flow with custom node/edge components (the theme seam).
- Add a **disposable sandbox route** `/maps/_proto` that mounts `MapCanvas` against a static
  fixture (no `+page.server.ts`, no loader, no auth — pure static). The real
  `/maps/[slug]/+page.svelte` stays the "coming soon" placeholder.
- Hand-roll a BFS layout (`$lib/map/layout.ts` or similar): rank by hops from `tab.roots`;
  L→R / T→B / radial variants for the one-shot "redo layout" action. No layout library.
- Placement persistence: positions persist across session restart via `localStorage[mapId]`,
  overlaid on the layout seed (`pos[id] = saved[id] ?? seed[id]`), and reconcile on
  graph change (dropped→forget, new→seed, kept→keep saved). Placement is NEVER graph truth.
- Add the map's new design tokens to `app.css`: class colours (`--c1..c6`, `--hs/--ls/--ns`)
  and mass colours (`--mass-fresh/half/critical`). (`--violet` already exists.)
- Render meaning **never by colour alone**: class/security/static/wh-type already carry text;
  edge **mass** (fresh/half/critical) gets a text label so it survives forced-colors and
  colourblindness (consistent with the shipped StatusIcon principle).
- Simulate SSE: a "receive update" affordance triggers the reconcile cycle so the prototype
  demonstrates ghost→reconcile without building real SSE.

Explicitly **NOT** in scope (deferred to the backend-model track / future changes):
the chain-map DB schema, real SSE, eve-scout polling, EoL liveness/archival, history-mode
event replay, server-side placement storage, and wiring the real `/maps/[slug]` route.

## Capabilities

### New Capabilities
- `map-canvas-prototype`: An interactive map canvas that renders a position-less combined
  graph via svelte-flow, with hand-rolled layout seeding, session-surviving placement
  persistence + graph-change reconciliation, and colour-independent encoding of map state.
  Deliberately a learning sandbox, not a durable feature contract.

### Modified Capabilities
<!-- none — the maps/ACL container capability is unchanged; map *contents* are still deferred. -->

## Impact

- **Dependencies:** adds `@xyflow/svelte` (frontend). No backend changes.
- **Frontend:** new `$lib/components/MapCanvas.svelte`, supporting `$lib/map/*`, a static
  fixture, the `/maps/_proto` sandbox route; new tokens in `src/app.css`. Governed by the
  `sveltekit-node` skill.
- **Disposability:** `/maps/_proto` is throwaway. When the model converges, the real route
  adopts the same `$lib` component (swap fixture → loader) and the sandbox is deleted. The
  reusable thing (`MapCanvas`) was never in the route.
- **Docs:** `openspec/AGENTS.md` route/component trees gain `/maps/_proto` + `MapCanvas`;
  reconcile in-change. No root-doc config/deploy facts change.
- **Verification (frontend-only change):** `pnpm test`, `pnpm run check`, `pnpm run test:e2e`
  — all run from `frontend/`.
