# map-canvas-prototype

## Purpose

A learning sandbox, not a durable feature contract. These requirements pin only the
behaviours that prove the interaction model and that a test can verify; the route and
fixture (`/maps/_proto`) are disposable. When the backend chain-map model converges, the
real `/maps/[slug]` route mounts the same reusable `MapCanvas` and this capability is
superseded.

## Requirements

### Requirement: Canvas renders a position-less combined graph

`MapCanvas` SHALL accept a combined graph (`{ systems, connections, single-root-per-tab }`)
that carries **no node coordinates** and render it via svelte-flow. Node and edge existence
SHALL be a pure function of the graph (reachability from `tab.root` over live connections),
NEVER derived from placement. Node positions SHALL be supplied by the layout seed and/or
saved placement, not read from the graph.

#### Scenario: Fixture with no coordinates renders nodes and edges
- **WHEN** the sandbox route `/maps/_proto` mounts `MapCanvas` with the static fixture (which contains no node positions)
- **THEN** every system reachable from the active tab's root renders as a node, and every live connection between rendered systems renders as an edge

#### Scenario: Existence is independent of placement
- **WHEN** a node is dragged to any position (or its saved position is cleared)
- **THEN** the set of rendered nodes and edges is unchanged — only its on-screen position moves

### Requirement: One-shot hand-rolled layout seeds positions from the tab root

The canvas SHALL compute seed positions by BFS rank (hop distance from the active tab's
single root), with left→right, right→left, top→bottom, and bottom→top variants selectable as
a one-shot "redo layout" action. Layout SHALL be deterministic (same graph + direction → same
positions) and SHALL use no external layout library. There is no persistent layout mode —
applying a layout is a one-shot action after which the map is user-positioned.

#### Scenario: Redo layout reseeds positions
- **WHEN** the user has dragged nodes and then selects a layout option (e.g. left→right)
- **THEN** the saved placement for the active tab is cleared and all nodes are repositioned by BFS rank from the root

#### Scenario: Layout is deterministic
- **WHEN** the same layout direction is applied twice to the same graph
- **THEN** the resulting positions are identical

#### Scenario: A new root means a new tab (no multi-root)
- **WHEN** a second root system is wanted on the map
- **THEN** it is added as its own single-root tab — a tab anchors at exactly one root, and the wildcard `*` tab is the place where multiple chains appear together

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
System class, security, and connection type SHALL render as text badges/labels. A static
SHALL render its **destination class** (HS/LS/NS/C1–C6/P) as text; the static's wormhole-type
code is NOT surfaced. Connection mass (fresh / half / critical) SHALL be conveyed by line
**thickness** AND a text cue on the edge label, not colour alone. Connection time-to-life
SHALL be conveyed by a hue-independent line **dash pattern** plus a shape-distinct glyph
whose accessible name states the time bucket; any breathing animation is decoration only and
its loss under `prefers-reduced-motion` SHALL NOT remove information.

#### Scenario: Mass is readable without colour
- **WHEN** an edge represents a half- or critical-mass connection
- **THEN** the edge line is rendered thinner than a fresh-mass edge AND the edge label includes a text cue distinguishing the mass state, independent of stroke colour

#### Scenario: Time-to-life is readable without colour or motion
- **WHEN** `prefers-reduced-motion` is active and a connection is in a low-time-to-life state
- **THEN** the state remains conveyed by the line's dash pattern and a shape-distinct glyph whose accessible text names the bucket, with no information carried only by the dropped breathing animation

#### Scenario: A static shows its destination class, not the wormhole type
- **WHEN** a system has a static wormhole
- **THEN** the node renders the static's destination class (e.g. `HS`, `C5`) as text, and does not render the wormhole-type code

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

### Requirement: Mass and time-to-life are independent, with a derived alert

A connection's encoding SHALL be resolved from two independent inputs — mass and
remaining-time — plus a derived alert, by a single pure function (no framework dependency,
unit-testable in isolation).

- **Mass** SHALL own line thickness and colour (fresh widest, critical thinnest, with a
  minimum width so the thinnest line keeps its dash texture). A user thickness control SHALL
  scale all widths uniformly, preserving the fresh > half > critical ordering.
- **Time-to-life** SHALL be bucketed from remaining-minutes into four model states
  (`stable`, `lt4h`, `lt1h`, `imminent`) which collapse to three visual tiers: above 4 h =
  calm, under 4 h = warning, and **under 1 h OR imminent = the same critical tier**. The
  precise four-state value SHALL remain available for accessible text.
- **The derived alert** (a breathing casing under the line) SHALL be a function of
  time-to-life ONLY: warning for `lt4h`, critical for `lt1h`/`imminent`, and none otherwise.
  Mass SHALL NOT contribute a casing.

#### Scenario: Fresh mass with low time draws attention as strongly as critical mass
- **WHEN** a connection is fresh mass but in the `imminent` (or `lt1h`) time bucket
- **THEN** it renders a critical-tier alert (red breathing casing + alert glyph), drawing the eye despite its full mass

#### Scenario: under-1-hour and imminent render identically
- **WHEN** two connections are in the `lt1h` and `imminent` buckets respectively
- **THEN** their dash pattern, glyph, and alert casing are identical (the critical visual), while their accessible text still distinguishes the two states

#### Scenario: Critical mass with healthy time does not glow
- **WHEN** a connection is critical mass but above 4 h of time remaining
- **THEN** it renders a thin red line with NO breathing casing (the alert is reserved for the time axis)

### Requirement: A colour-blind palette swaps only the mass hues

The canvas SHALL offer a colour-blind palette toggle that swaps ONLY the three mass hues to
a colour-blind-safe set. Line thickness, dash patterns, glyphs, motion, and the alert layer
SHALL be identical between palettes.

#### Scenario: Toggling the palette changes only the mass hues
- **WHEN** the colour-blind palette is toggled on
- **THEN** the canvas applies the colour-blind mass hues and leaves every non-mass-hue channel (thickness, dash, glyph, alert) unchanged

### Requirement: Undetermined connection direction shows no endpoint marker

A connection whose direction is undetermined (neither endpoint signature is typed) SHALL
render with no direction arrowhead and no mid-edge direction marker; the line connects
normally. A direction arrowhead SHALL appear only when at least one endpoint signature types
the connection.

#### Scenario: Both ends unscanned renders a bare line
- **WHEN** a connection has no typed signature on either endpoint
- **THEN** the edge renders as a plain line with no arrowhead and no neutral mid-edge marker

### Requirement: Pochven is a first-class system class

The system-class model SHALL include Pochven (Triglavian space) as its own class, distinct
from null-sec and low-sec, rendered with its own text label and a dedicated colour token.

#### Scenario: A Pochven system renders as its own class
- **WHEN** a system is in Pochven
- **THEN** it renders a `P` class badge with the Pochven colour token, distinct from the NS and LS tiers
