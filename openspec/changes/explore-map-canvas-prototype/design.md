# Explore: map canvas prototype + chain-map data model

**Status:** EXPLORE ONLY — nothing built, nothing specced. This is a decisions+questions
ledger from an explore session (started 2026-06-13, paused for sleep). Resume from the
**Open forks** at the bottom. Not yet a proposal.

Related: [[project-chain-map-data-model]] (the leaning model this refines/extends),
[[project-sse-event-dispatch]] (the SSE seam this assumes but does not build),
[[project-maps-acls]] (the shared-map ACL model that already exists).

## What this exploration is actually about

The static wireframe `frontend/static/wireframes/map_canvas.html` (~2920 lines, 4 views,
full design notes) already nails the *visual* language. What it cannot do is prove the
*interaction* model — drag / pan / zoom / collision / layout — which only a real
svelte-flow build shows. So the prototype is the **living successor to the static
wireframe**: visually still malleable, interactive, dressed in the **real app design
tokens** (they already match the wireframe almost exactly — see below). When it works,
the static `map_canvas.html` becomes legacy. One source of truth, no drift.

Scope widened mid-session: the **data model is in-scope here too**. Authoring the
static fixture is the forcing function for deciding the chain-map model — the fixture
*is* a hypothesis about the model, and svelte-flow is the test rig.

## Canvas vs. backend model are SEPARABLE (key structural decision)

The canvas and the backend model can be worked **independently**, on a one-way contract:

```
  CANVAS  ── consumes a graph ──▶  renders/manipulates it
  (frontend, buildable NOW)        nodes/edges/connections, drag, collision,
                                   layout, theme, tabs, ghosts

  BACKEND MODEL ── produces a graph ──▶  discovery, storage, event-sourcing,
  (its own explore, later)               SSE, user prefs, eve-scout poll, EoL
                                         liveness, archive-vs-delete
```

- The canvas eats `{ nodes, edges, connections-with-metadata }`. It does NOT care how
  that graph came to exist. So it builds against a **static fixture** (a graph snapshot)
  with zero knowledge of the backend.
- A graph snapshot is **model-agnostic by construction** — any backend model must be able
  to *produce* a graph — so authoring fixtures commits us to nothing on the model. (This
  dissolves the earlier "fixture silently bakes the model" worry: it only bites if the
  canvas reaches BACK into model concerns. Keep the contract one-way.)
- Iterating the canvas **informs but does not dictate** the model: "I need to know a
  connection's EoL state to render the red pulse" tells the backend *what to expose*, not
  *how to store/compute it*.
- Already true today: the `/maps` container + ACL resolver EXIST and are archived
  ([[project-maps-acls]]); **map *contents*** (connections/sigs/events) are explicitly
  deferred by the maps spec. So the model is genuinely separate, later work — the canvas
  need not wait.

**Two tracks, different speeds.** The Open forks below split along this seam:
- CANVAS forks: nudge persistence (A/B), layout-lib choice, a11y/forced-colors rendering.
- MODEL forks: EoL liveness (lazy/eager), event-sourcing, eve-scout poll mechanics,
  archive-vs-delete. Track 1 can start without resolving Track 2.

## The UI is the requirements-gathering instrument (extends the seam)

The whole frontend — canvas AND sidebar — is a **renderer of state**, and building it is
how we **discover what state the model needs**. The UI probes the model, not the reverse.

```
   render some UI against fictional/local state
        │  "to show this, I'd need to know X"
        ▼
   X becomes a CANDIDATE field
        ├── keep  → model must store/expose X
        └── cut   → decided we don't need X; recorded as deliberately-cut
```

- **Sidebar is just another editor of the combined graph.** Adding a signature →
  appears in the sig table → binds to a node → when typed as a WH, becomes an edge →
  the edge is part of a connection. All of that is **local state transitions on the
  fixture**; none needs the backend yet. (Same reconcile model as the canvas: mutate
  localState → `server ∪ localState` re-derives → svelte-flow re-renders.)
- System **detail / notes**, and node badges like **owner-corp**, are visual state now.
  Rendering owner-corp is a *probe*: "do we want this enough to store it?" — and the
  answer may legitimately be **no** (cut it; cheaper to cut a badge than a column +
  event + SSE field already built on it).
- So the fixture is a **scratchpad of candidate state**, and the shape it settles into
  IS the first real draft of the map-*contents* schema — earned by use, not guessed.

**Discipline — probes must stay reversible.** The method only works if fixture fields are
provisional: add freely, cut freely. Don't let a provisional field harden into an assumed
contract (don't build 3 features on owner-corp before owner-corp has earned its place).
Keep a running **candidate-fields list** below; the model track inherits a *decided* list,
cut fields recorded as deliberately-cut, not forgotten.

### Candidate fields surfaced by the canvas/sidebar (running list)
- sig → edge binding (sig ID per side, type) — **core**
- connection metadata: origin (manual/eve_scout), mass, eol state, sig endpoints — **core**
- connection `wh_type` (e.g. C364/D845, shown as edge-label text) — **core** (confirmed by wireframe)
- connection `mass` as an **enum with a text label** (fresh/half/critical), NOT colour-only —
  **core** (Track-1 a11y finding: mass was the one colour-only hole; render needs text)
- system detail / notes — likely
- owner-corp on node — UNDECIDED (probe; may cut)
- _(append as the prototype surfaces more)_

## The spine (three axes — keep them separate)

```
EXISTENCE   = the combined graph. node/edge existence is a pure function of it,
              NEVER derived from placement.
              combined = server-state  ∪  localState
              render   = reachable(tab.root, live_connections) ∪ local ghosts

PLACEMENT   = pure presentation, ZERO graph weight. layout preference (one-shot
              seed) + drag + collision-repel. Persistence = OPEN (fork A/B).

STYLE       = theme. custom svelte-flow node/edge components ARE the theme seam;
              a theme swaps/parameterises those components. Overrides app defaults.
```

The one rule that keeps interactions honest:

> A gesture may affect **placement** or **style** freely, but must **never silently
> assert graph truth** (a system, a connection). Graph truth is scanned reality, not
> a side-effect of a drag.

## Decisions reached this session

- **We never store node positions as graph truth.** Existence is a function of the
  combined graph; svelte-flow gets placement from a layout strategy + user drags, not
  from stored coordinates. (Reconciles with — does not contradict — the wireframe's
  "always user-positioned, one-shot auto-layout, no persistent layout mode" note.)

- **Combined-graph model.** Client holds: (1) **server state** (shared, real, arrives
  via SSE later; simulated by the fixture now) and (2) **localState** (this browser:
  optimistic / not-yet-real systems, e.g. right-click "add system"). svelte-flow
  renders `server ∪ localState`.

- **Reconciliation, not promotion.** A locally-added system (e.g. `J999999`) lives in
  localState. When a server/SSE update arrives containing it, the reconcile cycle
  **removes it from localState** (server is now authoritative). Because render is the
  union, the drop is visually seamless — no explicit "promote" step.

- **Right-click → add system** is local-only. It appears to other users only once a
  real connection reaches it from a tab root (i.e. it enters `reachable()` in shared
  server state).

- **Tabs are local browser state, NOT persisted server-side** (matches wireframe).
  A tab = a root filter over the shared connection graph.

- **Tabs are SINGLE-ROOT** (REVISED 2026-06-17 — was multi-root). `tab.root` is one
  system id, not a set. Multi-root was dropped as unnecessary Tripwire-flavoured
  complexity: wanting a second root just means opening another tab, and the layout was
  handling a multi-root rank poorly. A **wildcard `*` tab** (no root, `isWildcard`) is the
  one place multiple chains appear together — it shows every system with a live mapped
  connection and synthesises a layout seed from the first present system (anything
  unreachable from it, e.g. a disconnected second chain, parks in the gutter).
  - *Superseded:* the earlier "`tab.roots` is a SET / 1..N roots / min-hop rank across the
    root set" direction (and its layout test + fixture multi-root "Deep" tab) is gone.

- **eve-scout (Thera/Turnur) is merged BACKEND-side**, not a render-time client merge.
  A backend job polls `https://api.eve-scout.com/v2/public/signatures` and merges into
  the DB, so by the time the client sees them they're ordinary server-state connections,
  available to all maps at once. The frontend never talks to eve-scout.

- **Connections carry an `origin`** (`manual` / `eve_scout`). The wildcard tab's
  "include/exclude Thera/Turnur" is therefore a **client-side filter on origin**, not a
  fetch toggle.

- **proximity-connect: REJECTED.** No sensible use here — drag-near-to-auto-create-edge
  would fabricate an unscanned wormhole (violates the graph-truth rule). svelte-flow
  examples are illustrations of its toolbox, not a contract.

- **collision-repel: safe to adopt.** It's a custom `resolveCollisions()` run one-shot
  on `onnodedragstop` (NOT a built-in) — pure placement, no graph weight.

- **svelte-flow capability note:** collision-repel and proximity-connect are *patterns
  built on the drag callbacks* (`onNodeDrag` / `onNodeDragStop` + custom logic), not
  toggles. svelte-flow gives the event surface; behaviour is ours. Custom nodes/edges
  are plain Svelte components → that's the theme seam.

- **`@xyflow/svelte` is NOT yet a frontend dependency.** Adding it is itself a decision.

## Prototype home (decided: separate sandbox route)

- The **canvas is a component in `$lib`** (e.g. `MapCanvas.svelte`), real & reusable.
- A **disposable sandbox route** (e.g. `/maps/_proto`) mounts it with a static fixture
  (no `+page.server.ts`, no loader, no auth — pure static).
- Real `/maps/[slug]/+page.svelte` stays the 16-line "coming soon" placeholder until the
  model converges. Nothing to reconcile later: delete the sandbox, point the real route
  at the same `$lib` component, swap fixture → loader. The reusable thing was never in
  the route.
- Governed by the `sveltekit-node` skill like any route. (Minor open: guard `_proto` out
  of prod, or leave it — at wormhole scale, leaving it is probably fine.)

## Fixture shape (the data contract under test)

A snapshot of the combined graph, two layers kept strictly separate:

```
server-state:  { systems:     [{ id, class, effect, statics, ... }],
                 connections: [{ a, b, origin, mass, eol_at, sig_a, sig_b, ... }],
                 single-root-per-tab }
localState:    starts empty (or one seeded ghost to demo right-click-add)
render = reachable(tab.root, live_connections) ∪ localState
```

NO positions in the fixture. SSE is *simulated* (a "receive update" button triggers
reconcile) — the prototype demonstrates ghost→reconcile without building real SSE.

## Design-token reconciliation (cheap — already aligned)

`map_canvas.html` was authored FROM the app tokens. `--space-*`, `--slate-*`,
`--sky/emerald/amber/red`, JetBrains Mono all match `frontend/src/app.css` exactly.
Two real gaps:
1. Wireframe hard-codes **class colours** (`--c1..c6`, `--hs/ls/ns`), **mass colours**
   (`--mass-fresh/half/critical`), and **`--violet`** — none exist in `app.css`. These
   are genuinely new tokens the map needs (a real token decision).
2. App has **forced-colors + dark high-contrast** overrides (commit `eba31ca`); the
   wireframe knows nothing of them. svelte-flow rendering through a11y modes is unproven.

## TRACK 1 (canvas) FORKS — RESOLVED 2026-06-16

The three canvas forks are closed. Implementable change: **`build-map-canvas-prototype`**
(separate change dir; this ledger stays the explore record + Track-2 home).

- **Fork 1 — placement persistence: RESOLVED → B (survives session restart).**
  Not "A vs B": placement is *our own convenience arrangement* and SHOULD survive a
  restart (never graph truth). svelte-flow persists nothing itself (verified — it only
  exposes `setNodes`/`useNodes`), so we own save/restore regardless. **Sandbox:**
  `localStorage[mapId]`, reconcile on graph change — `pos[id] = saved[id] ?? seed[id]`;
  dropped id → forget, new id → seed, kept id → keep saved. The storage *backend*
  (localStorage vs per-user server state) is a **Track-2** decision, deferred with the model.

- **Fork 2 — layout: RESOLVED → hand-roll BFS + barycenter ordering (no lib).**
  svelte-flow has NO built-in auto-layout (verified — *"we believe you know your app's
  requirements best… choose the best tool"*; it renders positions you give it). Our graph
  is root-anchored (`tab.root`, single) and shallow: `rank = BFS hops from the root`; L→R
  `x=rank,y=sibling`; T→B swaps (RL/BT mirror). **`@xyflow/svelte` stays the ONLY map dep.**

  - **ELK.js spiked then REJECTED (2026-06-17).** Tried `elkjs` `layered` for crossing
    reduction; the layouts were clean, BUT ELK's `layout()` is a **Promise**, and going
    async dragged in disproportionate machinery for a shallow-tree seed: an epoch guard for
    stale resolves, a `FitOnSeed` child to re-`fitView` after late positions, `fitSignal`
    plumbing, and `expect.poll`/settle reworks across the e2e suite — plus an initial-load
    fit bug (nodes mount at origin before ELK resolves). Per CLAUDE.md proportionality, the
    async blast radius outweighed "reduce crossings on a shallow root-anchored graph". Reverted
    whole, dropped the dep.
  - **What actually fixed crossings: a synchronous barycenter sibling-ordering pass**
    (`orderRanks` in `layout.ts`). The crossing problem was only ever *sibling order within a
    rank* (was raw `graph.systems` insertion order). Order each rank by the mean index of its
    neighbours in the adjacent rank; 2 down/up sweeps converge on shallow chains. Seats children
    beside parents, uncrosses edges — ~visually on par with ELK's `layered` on the fixture, ~40
    lines, pure + sync + deterministic, zero deps, no fit machinery (the `<SvelteFlow fitView>`
    prop just works because positions exist at mount).
  - **`radial` DROPPED (2026-06-17, user-decided).** Only the four cardinal flows remain
    (LR/RL/TB/BT). Removed from `LayoutDirection`, `positionFor`/`placeGutter`, `placeIncoming`,
    the sidebar button, and the `map_proto_layout_radial` i18n key ×3 locales.

- **Fork 3 — a11y rendering: RESOLVED → badge text carries meaning, colour decorates.**
  Matches the shipped StatusIcon principle ([[project-status-icon]]). Wireframe already
  pairs class/sec/statics/wh-type with TEXT (C3, HS, C5a, C364) → forced-colors-safe.
  EoL already has a `⚠` glyph (survives flatten + reduced-motion; pulse is pure decoration).
  **Only colour-only hole was edge MASS** → render fresh/half/critical as text on the edge
  label + colour. Actual `forced-color-adjust` CSS is a real-route concern, deferred.

### Unifying data flow (how the three forks compose)
```
  layoutSeed(graph)                       ← Fork 2: pure fn, graph → positions  (the FLOOR)
       │ default
       ▼
  pos[id] = saved[id] ?? layoutSeed(g)[id]  ← Fork 1: persistence OVERLAY on the seed
       │
       ▼
  svelte-flow renders pos                 ← Fork 3: nodes/edges as themed components,
                                            meaning never colour-only
```
"Redo layout" = clear the overlay for the tab + re-seed. Reconcile-on-graph-change re-runs
this with the new node set. One flow, three slots.

### New tokens the map needs (real token decision, captured for the build)
None exist in `app.css` yet (`--violet` now DOES exist — ledger was stale):
- class colours `--c1:#60a5fa --c2:#34d399 --c3:#a78bfa --c4:#f472b6 --c5:#fb923c --c6:#f87171`
- security colours `--hs:#4ade80 --ls:#facc15 --ns:#fca5a5`
- mass colours `--mass-fresh:#22c55e --mass-half:#f59e0b --mass-critical:#ef4444`

## DECISION (2026-06-17): the connection is a "propagation group" — the unit of transport, identity, and render

Emerged while solving edge **direction** legibility for an ELK left-to-right layout (the
arrowhead-at-a-shared-perimeter-point ambiguity — see the rejected approaches below). The
resolution reframes what an edge *is*.

**A connection renders and travels as one composite triple:**

```
  [ sig_a? ] ——— conn ——— [ sig_b? ]      (sig_a and sig_b each independently nullable)
```

The two endpoint sig **pills** and the line between them are ONE thing — a *propagation
group* — not a line that happens to have labels parked near its ends. This matches the
data model we already have (`Connection = { a:{system,sig}, b:{system,sig}, … }`,
`sig: Signature | null`); the shift is in how we render/reason, and in naming the wire
contract.

### Direction is INTRINSIC to the group, never an endpoint arrowhead

Direction is a property of the **signatures**, not the line: a hole is a named type
(`H296`) on one side and `K162` on the other, so the *typed* sig orients the whole group.
`k162End(conn)` already derives this and returns `'a' | 'b' | null`, covering all four
null/typed combinations exactly:

| sig_a | sig_b | direction |
|-------|-------|-----------|
| typed | typed | known (either) |
| typed | null  | known |
| null  | typed | known |
| null  | null  | **unknown — render line only, no pills, no cue (honest)** |

So direction is shown **on the sig pill** (the thing that *causes* it), as a
shape/position cue (chevron-pill / caret), **never colour** (keeps the "meaning never
colour-only" rule). The both-null case shows no pills — absence of pills IS the "we don't
know direction" signal (no neutral mystery marker; that was already removed).

**This dissolves three earlier threads at once:**
- the endpoint **arrowhead** (`markerEnd`/`markerStart` + `MarkerType.ArrowClosed` in
  MapCanvas) becomes redundant for direction → removal candidate;
- the **ELK-LR convergence problem** (many edges sharing one node-side anchor pile up
  ambiguous arrowheads) evaporates — there's no endpoint arrowhead by construction;
- the **endpoint-spread** idea (distributing landings along a node side, à la the
  resize-handle screenshot) drops from "needed for direction legibility" to *optional
  polish* — the pills already disambiguate per-line even when the lines converge (this is
  what Wanderer's pills do; its edges carry no arrowheads either).

Rejected on the way here: (a) endpoint-spread as the *primary* fix — heavy reconciliation
(per-(node,side) grouping, corner-flip hysteresis, crowded-side degradation); (b)
mid-edge direction chevron — works but puts the cue where the cause isn't.

### The group is the SSE TRANSPORT unit (the load-bearing reason)

The backend doesn't push a line + two separate sig labels. When a wormhole changes, the
atomic thing that changed is the **whole group** `sig_a? · conn · sig_b?`. So the group is
the unit of **change**, **identity**, and therefore **render** — render = transport.
Modelling the edge as anything else forces decompose-on-receive / recompose-on-render,
which is pure friction and a drift source (orphaned sig, wrong-side update).

Contract decisions:

1. **Stable identity = `conn.id`.** A sig getting identified (null → typed) arrives as the
   SAME group id with the field changed — NOT a new group (else the client churns/re-places
   it). Sigs have no independent wire identity; a sig *is* "the K162 end of conn X".
2. **Whole-replace per group (DECIDED).** An update resends the entire triple; the client
   does `byId[group.id] = group`. No deltas/patches — a group is tiny, replace-by-id is
   trivially idempotent and correct, and it suits wormhole scale. (Diffing old↔new group is
   only needed later for *cosmetic* transitions like a "newly identified" highlight — not
   for correctness.)
3. **`MapEvent` already carries this** — the `connection` in `add-system`/`add-connection`
   already includes `a.sig`/`b.sig`. Make it intentional: events create/replace/remove
   **groups** by `conn.id`; there is no standalone "sig event".

### Type-system implication

Introduce an explicit propagation-group view-type (`sig_a? · conn · sig_b?`) as the unit
the edge component consumes AND (close to) the SSE event payload type — one type spans
wire → store → render, with the null-handling type-enforced. (Vs. keeping the raw
`Connection` and treating the triple as an implicit rendering convention — rejected as less
honest to the transport reality.)

**Status of this decision:** captured, NOT implemented. Touches the SSE event contract
(backend, Track 2), the client store/`reconcile.ts`, and the edge/label render + marker
removal (frontend) together — wants its own change when picked up. Open sub-choices left
for that change: exact pill direction cue (chevron-pill vs caret vs typed-tag; mark-K162
vs mark-named end) and whether the arrowhead is removed outright or kept dormant during
transition.

## DECISION (2026-06-17): the chain-mapping workflow drives the data model (sig-first, stub-birth, sig↔sig merge)

Walking the actual "map the chain" workflow exposes what the model must represent. This is
the driving scenario for [[project-chain-map-data-model]]; the propagation-group decision
above is one projection of it.

### The workflow (what a mapper actually does)

1. **Undock in system_a, paste the in-game scan list into the tool.** The paste is a
   per-system SNAPSHOT, columns: `ID · Type · Group · Name · Signal% · Distance`. It mixes
   **Cosmic Signature** (must be scanned down; `%` climbs, Type/Group/Name fill in),
   **Cosmic Anomaly** (already 100%, no scanning), and **Structure / Ship / …** rows. A
   fresh sig is just an id at `0.0%` with blank Type/Group/Name.
2. **A row with `Group == "Wormhole"` spawns a propagation group**, that sig becoming
   `sig_a`, `system_a` = the pasted system (KNOWN). Far side null → a *dangling stub*. This
   is the NORMAL birth state of every connection, not an edge case.
3. **Warp to the hole, read the in-game info, update `sig_a`:** `type` (e.g. `E545`),
   **destination class** ("Null-security systems" → NS), **max ship size** (Large), the
   **lifetime** bucket ("Less than 1 day remaining") and **mass** bucket ("More than 50%
   remaining"). So the wormhole type lives on the sig and IMPLIES the dest class — a
   null-`system_b` stub can already render "→ NS". Mass/lifetime are transcribed from the
   in-game text and map onto the encoding buckets we already built.
4. **Jump through → now `system_b`'s id is known.** Paste system_b's scan list; its K162
   back to system_a shows as a wormhole sig (e.g. `ABC-123`), which would spawn ITS OWN
   stub group. The user then asserts the two are the same hole → **MERGE** (see below).

### Falls out of the workflow (model requirements)

- **`Signature` is FIRST-CLASS and system-owned**, not a field inside the connection. A
  system has `signatures[]`: `id · group (Data/Gas/Relic/Combat/Wormhole/…) · name · scan% ·
  distance`, and for wormholes also `type · dest_class · max_ship_size · mass · lifetime`.
  Most sigs are NOT wormholes (they feed the Signatures intel panel). A **connection is the
  projection of a wormhole-typed sig** (plus its discovered far end) onto an edge.
- **Stub-birth + far-side-fill lifecycle.** `system_a:string` non-null, `sig_a` present from
  birth; the far side (`system_b:string?`, `sig_b:Signature?`) fills over later steps. Dest
  class is known from `sig_a.type` even while `system_b` is null (render "→ NS").
- **Paste is a SYNC, not an insert.** Re-pasting a system's list diffs: add new, update %/
  type/name, and **a wormhole sig that DISAPPEARED from the paste = the hole collapsed →
  remove its connection.** So the Signatures panel drives connection lifecycle (birth AND
  death).
- **New attributes to carry:** `dest_class` and `max_ship_size` (both properties of the
  wormhole type), plus mass + lifetime buckets (already modelled as mass/TTL).

### MERGE: two stubs → one connection, bound SIG↔SIG (not sig→system)

When the far side is scanned independently, the same hole exists as two stub groups (one per
end). Merging them is a first-class op — and the bind MUST be **sig-to-sig**, because of the
parallel-hole case:

> system_a has TWO wormhole sigs to system_b (`WFH-937`, `XQK-201`); system_b therefore has
> two K162s back (`ABC-123`, `DEF-456`). "ABC-123 leads to system_a" is AMBIGUOUS — it can't
> say which of system_a's two holes it pairs with. The assertion must be "`ABC-123` is the
> far end of **`WFH-937`**."

So:

- **A connection's identity IS the sig-pair `(sig_a, sig_b)`** — the natural composite key.
  Parallel holes between the same system pair are distinct *because their sig-pairs differ*
  (this is also why we already bow parallel edges apart in render).
- **Merge binds `sig_b` → a specific `sig_a`.** The system_id is DERIVED from the chosen sig.
  Mechanics: delete the far-side stub group, fill the near group's far end
  (`sig_b`, `system_b`). Each parallel pair merges independently.
- **Invariant: each wormhole sig is in ≤ 1 connection.** Binding checks neither side is
  already paired. Merge is symmetric + idempotent (binding A→B ≡ B→A; twice = no-op).
- **Auto-inference is best-effort and IMPOSSIBLE for same-type parallels** (two identical
  holes to the same system can't be auto-paired) → **manual sig-to-sig binding is the
  required path**, with auto-suggest only when a candidate is unique (e.g. unique dest-class
  match, far end still null).
- **SSE consequence:** a merge is NOT a single whole-replace — it's a **remove (far stub) +
  replace (near group)**, ideally delivered together. So the event contract needs a remove
  alongside replace-by-`conn.id`; whole-replace still governs the per-group payload.

### Open sub-questions for the eventual change

- **Merge trigger:** manual sig-to-sig only, auto-suggest + confirm, or full auto when
  unambiguous? (Leaning: manual default, auto-suggest when unique.)
- **Collapse detection ownership:** does pasting a sig list own connection death
  (sig-vanished → remove), or is collapse a separate signal (EoL job / eve-scout)? Ties to
  the EoL-liveness fork below.
- **Multi-user races:** two pilots scan the two ends; both stubs land on the server before
  either merges. The merge is a human assertion that reconciles them — needs a defined
  resolution (and an UNMERGE / mis-merge undo, probably out of v1 scope).
- **Distance/scan%/anomaly/structure rows:** how much of the non-wormhole paste the tool
  persists vs. shows transiently in the Signatures panel.

**Status:** captured, NOT implemented. This is the chain-map data model's driving scenario;
it and the propagation-group decision are the same future change-family (model + SSE
contract + client store + render).

## TRACK 2 (backend model) FORKS — STILL OPEN (resume here for the model session)

- **localState never confirmed:** if I locally add `J999999` and a real connection never
  reaches it, does it live in localState forever? TTL / manual dismiss / persist-until-cleared?

- **Wildcard eve-scout filter** is a client origin-filter — confirm UX (per-tab toggle).

- **EoL liveness (from [[project-chain-map-data-model]]):** lazy (now < eol_at + jitter,
  filter in reachability) vs eager job-delete vs hybrid; soft-archive vs hard-delete on
  expiry; what the jitter models. Wireframe shows EoL as a red *pulsing-but-present* edge
  → implies two states (`eol_warning` vs `expired`), not instant vanish.

- **History mode (surfaced 2026-06-16, not previously in ledger):** the wireframe has a
  per-tab clock button → a scrubber rail that *replays the event log* to reconstruct map
  state at a point in time (read-only, dimmed canvas, timestamp banner). This is a Track-2
  concern (event-sourcing), but it does NOT threaten the one-way contract: it just means the
  canvas must render an arbitrary point-in-time graph snapshot — which the static-fixture
  approach already satisfies (a fixture *is* a snapshot). Records a model requirement
  (durable event log), not a canvas blocker.

- **Storage backend for placement** (deferred from Fork 1): localStorage vs per-user server
  state vs IndexedDB; orphan-key cleanup when a node id leaves the chain (non-problem at
  wormhole scale — sweep on load or ignore).
