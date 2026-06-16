# Tasks — build-map-canvas-prototype

Frontend-only change. Governed by the `sveltekit-node` skill (invoke it before writing the
first line of Svelte/TS). All commands run from `frontend/` (no pnpm workspace root).

## 1. Dependency + tokens

- [ ] 1.1 Add `@xyflow/svelte` to `frontend` (pnpm). Confirm it's the ONLY new dependency.
- [ ] 1.2 Add map design tokens to `src/app.css`: class colours `--c1..--c6`, security
  colours `--hs/--ls/--ns`, mass colours `--mass-fresh/--mass-half/--mass-critical`
  (values in the explore ledger). `--violet` already exists — do not re-add. Add their
  high-contrast (`prefers-contrast: more`) variants alongside, matching the existing block.

## 2. Graph contract + fixture

- [ ] 2.1 `src/lib/map/types.ts`: define the position-less combined-graph contract
  (`System`, `Connection` with `origin`/`mass`/`eol`/`wh_type`/`sig_*`, `Tab` with a root
  SET, `CombinedGraph = { systems, connections }`, `LocalState`).
- [ ] 2.2 `src/lib/fixtures/map-canvas.ts`: a static snapshot exercising every render state
  (classes C1–C6 + HS/LS/NS, fresh/half/critical mass, an EoL connection, a multi-root tab,
  the wildcard `*` tab, a seeded ghost in local state). NO node positions.
- [ ] 2.3 Provide a *second* server-state snapshot (or a delta) for the simulated update so
  the reconcile path (new node, departed node, ghost→confirmed) can be demonstrated.

## 3. Layout (Fork 2 — hand-rolled BFS, no lib)

- [ ] 3.1 `src/lib/map/layout.ts`: BFS rank from `tab.roots` (min hop across the root set);
  `layoutSeed(graph, tab, dir)` for `dir ∈ {LR, TB, radial}`; deterministic sibling ordering;
  park disconnected/ghost nodes in a visible gutter rank.
- [ ] 3.2 Unit-test layout: determinism (same input → same output), multi-root min-hop rank,
  the three directions, ghost parking.

## 4. Placement + reconcile (Fork 1)

- [ ] 4.1 `src/lib/map/placement.ts`: thin seam over `localStorage[mapId]` — `load(mapId)`,
  `save(mapId, positions)` (debounced), `clearTab(mapId, tab)`. Keep it swappable (the real
  backend is a later Track-2 decision).
- [ ] 4.2 `src/lib/map/reconcile.ts`: `combined = serverState ∪ localState`;
  `pos[id] = saved[id] ?? layoutSeed(...)[id]`; on graph change — dropped→forget saved,
  new→seed, kept→keep saved; local-only system removed from localState once server confirms.
- [ ] 4.3 Unit-test reconcile: survive-restart (saved beats seed), new-takes-seed,
  departed-forgotten, ghost→confirmed (no duplicate/flicker), existence independent of placement.

## 5. Canvas + custom components (Fork 3 — meaning never colour-only)

- [ ] 5.1 `src/lib/components/MapCanvas.svelte`: mounts svelte-flow against the combined
  graph; wires drag → `onnodedragstop` → save + one-shot `resolveCollisions()` (custom, NOT
  built-in proximity-connect); pan/zoom; minimap; the layout slide-out (LR/TB/radial);
  tabs (local state, multi-root, wildcard `*`); a "receive update" affordance for simulated SSE.
- [ ] 5.2 `src/lib/components/map/SystemNode.svelte`: class/security/static badges as TEXT +
  colour decoration; root indicator; ghost styling for local-only systems.
- [ ] 5.3 `src/lib/components/map/ConnectionEdge.svelte`: wh-type label; mass as a TEXT cue
  (fresh/half/critical) + colour; EoL `⚠` glyph + pulse (decoration only, drops under
  reduced-motion with no info loss).
- [ ] 5.4 Component-test the encoding rules: mass readable without colour; EoL readable
  without motion; class/sec/statics render as text.

## 6. Sandbox route

- [ ] 6.1 `src/routes/maps/_proto/+page.svelte`: mount `MapCanvas` with the fixture. NO
  `+page.server.ts`, NO loader, NO auth — pure static. Real `/maps/[slug]` stays untouched.
- [ ] 6.2 e2e (Playwright): load `/maps/_proto`; nodes/edges render; drag a node and reload →
  position survives; redo-layout reseeds; "receive update" reconciles a ghost into a real node.

## 7. Docs (in-change, per project rules)

- [ ] 7.1 Update `openspec/AGENTS.md`: add `/maps/_proto` to the route tree and `MapCanvas`
  (+ `$lib/map/*`, custom node/edge components) to the component tree; note the new map tokens.
- [ ] 7.2 Root docs: no config/deploy/setup facts change (sandbox route, no env vars) — confirm
  nothing in `frontend/README.md` needs reconciling; touch only if a documented fact moved.

## 8. Verification (frontend-only change — ALL THREE must pass, run from `frontend/`)

- [ ] 8.1 `pnpm test` — Vitest unit/component tests
- [ ] 8.2 `pnpm run check` — svelte-check (type-check + paraglide compile)
- [ ] 8.3 `pnpm run test:e2e` — Playwright e2e
