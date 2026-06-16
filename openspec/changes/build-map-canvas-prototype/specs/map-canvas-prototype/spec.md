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

### Requirement: Placement persists across session restart and reconciles on graph change

Manual node positions ("nudges") SHALL persist across a session restart (sandbox backend:
`localStorage` keyed by map). On load, a node's position SHALL be its saved position if one
exists, otherwise the layout seed. Persisted placement is a personal convenience cache and
SHALL NEVER be treated as graph truth. When the graph changes, placement SHALL reconcile:
nodes that left the graph forget their saved position, new nodes take the layout seed, and
nodes that remain keep their saved position.

#### Scenario: Nudges survive a reload
- **WHEN** the user drags nodes and then reloads the sandbox route
- **THEN** each dragged node reappears at its saved position rather than its layout seed

#### Scenario: New node takes the seed, kept nodes keep their position
- **WHEN** a graph update adds a system while existing dragged nodes remain in the graph
- **THEN** the new system is placed by the layout seed and the existing dragged nodes keep their saved positions

#### Scenario: Departed node is forgotten
- **WHEN** a graph update removes a system that had a saved position
- **THEN** the node is no longer rendered and its saved position is not retained

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
