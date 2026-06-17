# map-canvas-prototype

A learning sandbox, not a durable feature contract. These requirements pin only the
behaviours that prove the interaction model and that a test can verify; the route and
fixture are disposable (see proposal "Disposability").

## ADDED Requirements

### Requirement: Canvas renders a position-less combined graph

`MapCanvas` SHALL accept a combined graph (`{ systems, connections, roots-per-tab }`) that
carries **no node coordinates** and render it via svelte-flow. Node and edge existence SHALL
be a pure function of the graph (reachability from `tab.roots` over live connections),
NEVER derived from placement. Node positions SHALL be supplied by the layout seed and/or
saved placement, not read from the graph.

#### Scenario: Fixture with no coordinates renders nodes and edges
- **WHEN** the sandbox route `/maps/_proto` mounts `MapCanvas` with the static fixture (which contains no node positions)
- **THEN** every system reachable from the active tab's roots renders as a node, and every live connection between rendered systems renders as an edge

#### Scenario: Existence is independent of placement
- **WHEN** a node is dragged to any position (or its saved position is cleared)
- **THEN** the set of rendered nodes and edges is unchanged — only its on-screen position moves

### Requirement: One-shot hand-rolled layout seeds positions from tab roots

The canvas SHALL compute seed positions by BFS rank (hop distance from the active tab's root
set), with left→right, top→bottom, and radial variants selectable as a one-shot "redo
layout" action. Layout SHALL be deterministic (same graph + direction → same positions) and
SHALL use no external layout library. There is no persistent layout mode — applying a layout
is a one-shot action after which the map is user-positioned.

#### Scenario: Redo layout reseeds positions
- **WHEN** the user has dragged nodes and then selects a layout option (e.g. left→right)
- **THEN** the saved placement for the active tab is cleared and all nodes are repositioned by BFS rank from the roots

#### Scenario: Layout is deterministic
- **WHEN** the same layout direction is applied twice to the same graph
- **THEN** the resulting positions are identical

#### Scenario: Multi-root tab ranks from the nearest root
- **WHEN** the active tab has more than one root system
- **THEN** each node's rank is its minimum hop distance across the root set

### Requirement: Placement is ephemeral and reconciles in-place on graph change

Node positions SHALL be ephemeral: the map is laid out ONCE on load (layout seed), and a
refresh re-lays-out from scratch — manual nudges and incremental placements are NOT
persisted (Fork 1 was REVERSED in implementation; see the change design). Placement SHALL
NEVER be treated as graph truth. While the session is live, positions are owned by
svelte-flow (drag mutates them); when the graph changes via an event, placement reconciles
in-place: a new node takes its incrementally-computed slot, nodes that remain keep their
current (live) position, and a removed node drops with its edges.

#### Scenario: A reload re-lays-out (nudges do not survive)
- **WHEN** the user drags nodes and then reloads the sandbox route
- **THEN** the map is laid out afresh from the seed; the dragged positions are not restored

#### Scenario: New node placed incrementally, kept nodes keep their live position
- **WHEN** a graph event adds a system while existing (possibly dragged) nodes remain
- **THEN** the new system is placed one flow-step from its anchor (then collisions ripple) and the existing nodes keep their current positions

#### Scenario: Departed node drops with its edges
- **WHEN** a graph event removes a system
- **THEN** the node and its connected edges are no longer rendered

### Requirement: Map state is never encoded by colour alone

Every piece of map state that carries meaning SHALL be conveyed by text or shape in addition
to colour, so it survives forced-colors mode and is distinguishable without colour vision.
System class, security, statics, and connection type SHALL render as text badges/labels.
Connection mass (fresh / half / critical) SHALL render a text cue on the edge label, not
colour alone. End-of-life connections SHALL carry a non-colour glyph (e.g. `⚠`); any pulse
animation is decoration only and its loss under `prefers-reduced-motion` SHALL NOT remove
information.

#### Scenario: Mass is readable without colour
- **WHEN** an edge represents a half- or critical-mass connection
- **THEN** the edge label includes a text cue distinguishing the mass state, independent of stroke colour

#### Scenario: EoL is readable without motion
- **WHEN** `prefers-reduced-motion` is active and a connection is end-of-life
- **THEN** the end-of-life state remains conveyed by the `⚠` glyph (and label text), with no information carried only by the dropped pulse

### Requirement: Local-only systems reconcile away when the graph confirms them

A locally-added system (e.g. a right-click "add system") SHALL live in local state and render
as a ghost until a real connection reaches it from a tab root. When a graph update arrives
containing that system, it SHALL be removed from local state and rendered from server state
instead, with no explicit "promote" step (render is the union of server state and local
state, so the transition is seamless). The prototype SHALL simulate this update without real
SSE (e.g. a "receive update" affordance that swaps server state and runs reconcile).

#### Scenario: Ghost becomes a real node on update
- **WHEN** a system exists only in local state and a simulated graph update arrives containing that system as server state
- **THEN** the system is rendered from server state, is removed from local state, and its rendering does not flicker or duplicate
