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
            EPHEMERAL — laid out once on load, placed incrementally per SSE event,
            never persisted (a refresh re-lays-out). [Fork 1 REVERSED — see below.]

STYLE     = theme. custom svelte-flow node/edge components ARE the theme seam.
```

> One rule that keeps interactions honest: a gesture may affect **placement** or **style**
> freely, but must **never silently assert graph truth**. A system/connection is scanned
> reality, not a side-effect of a drag. (→ proximity-connect REJECTED.)

## Unifying data flow

```
  INITIAL LOAD: layoutSeed(graph, dir)        pure fn, graph → positions     (one-shot)
       │ seed nodes once
       ▼
  svelte-flow owns positions (drag mutates them; nothing persists them)
       ▲
       │ per SSE event
  LIVE: placeIncoming(anchorPos, dir) → new node one flow-step out, then
        resolveCollisions over the whole graph ("let it ripple")            (incremental)
       │
       ▼
  custom node/edge components render          meaning never colour-only         (Fork 3)
```

- **"Redo layout"** = recompute the seed for the active tab in a new direction and apply it
  to the live nodes (and update the flow direction future adds step along).
- **SSE event on graph change** = mutate the existence union, place incrementally:
  `add-system → placeIncoming + ripple`, `remove → drop node + edges`. No whole-map
  re-layout. Render is always the union, so a confirmed ghost dedupes seamlessly.

## Module shape (governed by `sveltekit-node`)

```
frontend/src/
  lib/
    components/MapCanvas.svelte        reusable canvas; consumes a position-less graph
    components/map/                    custom svelte-flow node/edge components (theme seam)
      SystemNode.svelte                class/sec/static badges (text + colour)
      ConnectionEdge.svelte           wh-type + mass label, EoL ⚠, dash/colour decoration
    map/
      layout.ts                        hand-rolled BFS seed (L→R / T→B / radial) — one-shot
      reconcile.ts                     combined = server ∪ localState (existence union only)
      place-incoming.ts                where an SSE-added node lands (one flow-step from anchor)
      resolve-collisions.ts            official @xyflow repel (drag-stop + after an add)
      types.ts                         graph contract + MapEvent SSE union (System, Connection…)
    fixtures/map-canvas.ts             initialGraph + ordered updateEvents (the data under test)
  routes/maps/_proto/+page.svelte      disposable sandbox; mounts MapCanvas with the fixture
```

`/maps/_proto` has **no `+page.server.ts`** — pure static, no loader, no auth. The real
`/maps/[slug]` route is untouched (stays the placeholder).

## Fork resolutions (the decisions this change implements)

- **Fork 1 — placement persistence → B (survives restart).** svelte-flow persists nothing
  itself, so we own save/restore. Sandbox backend = `localStorage[mapId]`. The *backend*
  (localStorage vs per-user server state) is a Track-2 decision — keep `placement.ts` a thin
  seam so swapping the store later is one module.
  - **REVERSED in implementation → ephemeral placement (no persistence).** Adopting the
    Svelte Flow website model (one-shot initial layout + incremental per-event placement)
    made persistence the wrong default: positions are a transient, locally-arranged view of
    a live graph, not durable state worth restoring across a refresh. A reload re-lays-out
    from the server graph; drags live only in svelte-flow's session `nodes`. `placement.ts`
    and the placement overlay in `reconcile.ts` were deleted; `place-incoming.ts` +
    `resolve-collisions.ts` replace them. A per-user *saved arrangement* can return later as
    a deliberate Track-2 feature, but it is no longer the prototype's baseline.
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

No real SSE. The fixture ships an `initialGraph` (laid out once) plus an ordered list of
`updateEvents` (`MapEvent[]`). A sandbox "receive update" affordance replays the next event,
which the canvas applies incrementally — demonstrating ghost→confirm dedupe, incremental
add placement ("let it ripple"), and removal, all without any backend.

## Disposability contract

`MapCanvas` (and `$lib/map/*`, fixtures, custom nodes) are real and reusable. `/maps/_proto`
is throwaway. Convergence path: real `/maps/[slug]/+page.svelte` mounts the same `MapCanvas`,
swap fixture → loader, swap the sandbox `nextEvent` script → a real SSE stream of `MapEvent`s;
delete `_proto`. Nothing reusable lives in the route. (Minor open: guard `_proto` out of prod
or leave it — at wormhole scale, leaving it is fine.)

## Out of scope (Track 2 / future)

Chain-map DB schema, real SSE, eve-scout polling, EoL liveness/archival, history-mode event
replay, server-side placement store, wiring the real `/maps/[slug]` route. These remain OPEN
in the explore ledger.
