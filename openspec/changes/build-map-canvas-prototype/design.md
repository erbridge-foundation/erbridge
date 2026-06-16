# Design — build-map-canvas-prototype

Distilled from the explore ledger
(`openspec/changes/explore-map-canvas-prototype/design.md`). That ledger holds the full
reasoning and the still-open Track-2 (backend model) forks; this file is the architecture
the implementation follows. Read the ledger for *why*; read this for *what to build*.

## The spine (three axes — keep them separate)

```
EXISTENCE = the combined graph. node/edge existence is a pure function of it,
            NEVER derived from placement.
            combined = server-state ∪ localState
            render   = reachable(tab.roots, live_connections) ∪ local ghosts

PLACEMENT = pure presentation, ZERO graph weight. layout seed + drag + collision-repel.
            Persists across session restart (convenience), never graph truth.

STYLE     = theme. custom svelte-flow node/edge components ARE the theme seam.
```

> One rule that keeps interactions honest: a gesture may affect **placement** or **style**
> freely, but must **never silently assert graph truth**. A system/connection is scanned
> reality, not a side-effect of a drag. (→ proximity-connect REJECTED.)

## Unifying data flow

```
  layoutSeed(graph, dir)                      pure fn, graph → positions     (the FLOOR)
       │ default
       ▼
  pos[id] = saved[id] ?? layoutSeed(g,dir)[id]   persistence OVERLAY          (Fork 1)
       │
       ▼
  svelte-flow renders pos + custom node/edge components   meaning never colour-only (Fork 3)
```

- **"Redo layout"** = clear the saved overlay for the active tab, recompute the seed, apply.
- **Reconcile on graph change** = recompute the union with the new node set:
  `dropped id → forget saved pos`, `new id → seed`, `kept id → keep saved pos`. Visually
  seamless because render is always the union.

## Module shape (governed by `sveltekit-node`)

```
frontend/src/
  lib/
    components/MapCanvas.svelte        reusable canvas; consumes a position-less graph
    components/map/                    custom svelte-flow node/edge components (theme seam)
      SystemNode.svelte                class/sec/static badges (text + colour)
      ConnectionEdge.svelte           wh-type + mass label, EoL ⚠, dash/colour decoration
    map/
      layout.ts                        hand-rolled BFS seed (L→R / T→B / radial)
      reconcile.ts                     combined = server ∪ localState; placement overlay
      placement.ts                     localStorage[mapId] load/save (sandbox backend)
      types.ts                         fixture/graph contract (System, Connection, Tab…)
    fixtures/map-canvas.ts             static combined-graph snapshot (the data under test)
  routes/maps/_proto/+page.svelte      disposable sandbox; mounts MapCanvas with the fixture
```

`/maps/_proto` has **no `+page.server.ts`** — pure static, no loader, no auth. The real
`/maps/[slug]` route is untouched (stays the placeholder).

## Fork resolutions (the decisions this change implements)

- **Fork 1 — placement persistence → B (survives restart).** svelte-flow persists nothing
  itself, so we own save/restore. Sandbox backend = `localStorage[mapId]`. The *backend*
  (localStorage vs per-user server state) is a Track-2 decision — keep `placement.ts` a thin
  seam so swapping the store later is one module.
- **Fork 2 — layout → hand-rolled BFS, no lib.** Graph is root-anchored + shallow; the three
  wireframe layouts are all BFS-from-`tab.roots`. Keeps `@xyflow/svelte` the only new dep.
- **Fork 3 — a11y → text carries meaning, colour decorates.** Only colour-only hole was edge
  mass → add a fresh/half/critical text cue on the edge label. Defer the real-route
  `forced-color-adjust` CSS.

## Layout (Fork 2) detail

```
rank(node) = BFS hop distance from the nearest system in tab.roots
sibling(node) = stable index among nodes sharing a rank
L→R   : x = rank * DX,      y = sibling * DY
T→B   : x = sibling * DX,   y = rank * DY
radial: angle = sibling / count(rank) * 2π,  r = rank * DR  →  (r·cosθ, r·sinθ)
```
Deterministic (stable sibling ordering → same input, same layout). Multi-root tabs: BFS from
the root SET (min hop across roots). Disconnected/ghost nodes (localState not yet reached):
parked in a gutter rank so they're visible but clearly unreached.

## Encoding (Fork 3) detail

| State | Text cue (survives forced-colors) | Colour decoration |
|---|---|---|
| system class | `C1`..`C6` badge text | `--c1..c6` |
| security | `HS`/`LS`/`NS` badge text | `--hs/--ls/--ns` |
| statics | `C5a`/`HSa` badge text | (neutral) |
| connection type | `C364`/`D845` edge label | mass colour |
| connection **mass** | **fresh/half/critical text on edge label** (NEW) | `--mass-*` |
| EoL | `⚠` glyph in edge label | `--mass-critical`, pulse (decoration only) |

Pulse is decoration — under `prefers-reduced-motion` it drops with no information loss
(the `⚠` carries EoL). New tokens added to `app.css`: `--c1..c6`, `--hs/--ls/--ns`,
`--mass-fresh/half/critical` (values in the explore ledger).

## SSE simulation

No real SSE. A sandbox "receive update" affordance swaps the fixture's server-state to a
second snapshot and runs reconcile — demonstrating ghost→reconcile and graph-change
placement reconciliation without any backend.

## Disposability contract

`MapCanvas` (and `$lib/map/*`, fixtures, custom nodes) are real and reusable. `/maps/_proto`
is throwaway. Convergence path: real `/maps/[slug]/+page.svelte` mounts the same `MapCanvas`,
swap fixture → loader, swap `placement.ts` store → server/per-user; delete `_proto`. Nothing
reusable lives in the route. (Minor open: guard `_proto` out of prod or leave it — at
wormhole scale, leaving it is fine.)

## Out of scope (Track 2 / future)

Chain-map DB schema, real SSE, eve-scout polling, EoL liveness/archival, history-mode event
replay, server-side placement store, wiring the real `/maps/[slug]` route. These remain OPEN
in the explore ledger.
