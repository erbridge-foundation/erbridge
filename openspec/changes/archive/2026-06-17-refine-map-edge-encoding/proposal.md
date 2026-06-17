## Why

The map-canvas prototype shipped with a thin connection-edge encoding: a single mass colour
on the line and a text mass cue + `⚠` glyph on the label. That under-uses the line itself
and conflates two genuinely independent variables a wormhole crew reads constantly — how
much **mass** a hole has left vs. how much **time** (TTL) it has left. A fresh-mass hole can
be minutes from collapsing by age; the old encoding made that invisible. We also had no
colour-blind affordance for the mass hues, an `⚠`-only EoL signal, and a confusing neutral
mid-edge marker for undetermined direction.

This change is the iteration that pins a richer, accessible edge encoding on the prototype
(it is retroactive: the work already exists in the working tree on
`feat/map-canvas-prototype`). It also folds in two model corrections discovered while
building it (statics carrying a display shorthand instead of the real wormhole type; Pochven
missing from the system-class enum).

## What Changes

- **Mass owns line thickness + colour.** fresh `5px` / half `3px` / critical `2px` (floored
  so the thinnest line keeps its dash texture). The existing edge-thickness slider becomes a
  multiplier anchored at its default, so it still globally fattens/thins lines while
  preserving the fresh > half > critical ordering.
- **A TTL axis.** `Connection` gains `ttl_remaining_min`. Four model states
  (`stable | lt4h | lt1h | imminent`) collapse to **three visual tiers**
  (`calm | warning | critical`): above 4 h = calm, < 4 h = warning (amber), **< 1 h AND
  imminent = the same critical** (red). The four states stay distinct in the model (precise
  screen-reader text) but render identically once critical, because by the time it is
  imminent the urgency message is unchanged.
- **A derived, PURE-TTL alert layer.** Low-TTL edges get a translucent "breathing" casing
  (a second under-stroke, CSS-animated). Mass does NOT contribute a glow — the thin red line
  already conveys critical mass; the halo is reserved for the time axis so motion stays rare
  and meaningful.
- **Shape-distinct TTL glyphs** on the edge label (inline SVG clock/octagon), never
  colour-only; the precise state is the glyph's accessible name.
- **A colour-blind palette toggle** that swaps ONLY the three mass hues (Okabe-Ito) via a
  `data-edge-palette` attribute on the canvas wrapper — one-line swap, A/B comparable.
- **Undetermined direction renders nothing** at the endpoint (no arrow, no neutral mid-edge
  marker) — a missing arrow already reads as "direction not yet known".
- **`SystemStatic.code` → `wh_type`** (the real wormhole-type code, kept for later
  signature work); the node + sidebar now show only the static's **destination class**
  (HS/LS/C5…), not the type code.
- **Pochven added as `SystemClass = 'P'`** — its own distinct space type (not NS/LS), with a
  dedicated `--pochven` colour token.

## Capabilities

### New Capabilities
<!-- none — this iterates an existing capability -->

### Modified Capabilities
- `map-canvas-prototype`: the "map state is never encoded by colour alone" requirement is
  updated (mass now also drives thickness; EoL `⚠` glyph is replaced by the TTL glyph
  system; mass hues gain a colour-blind palette). A new requirement pins the
  mass/TTL/alert encoding and its accessibility properties. The system/static rendering
  requirement is updated (statics show destination class; Pochven is a class).

## Impact

- Frontend only; all changes are on the disposable `/maps/_proto` sandbox + the reusable
  `$lib` map components it mounts.
- New module `frontend/src/lib/map/edge-encoding.ts` (pure resolver, unit-tested).
- Touched: `lib/map/types.ts` (TTL types, `ttl_remaining_min`, `SystemStatic.wh_type`,
  `SystemClass 'P'`), `components/map/ConnectionEdge.svelte` + `ConnectionEdgeLabel.svelte`,
  `components/MapCanvas.svelte`, `components/map/MapSidebar.svelte`, `components/map/SystemNode.svelte`,
  `app.css` (alert/halo/pochven tokens + colour-blind palette block), the map fixture, and
  the three locale files (`messages/{en,de,fr}.json`).
- No backend, no new dependency.
- Verified: Vitest 396 / `pnpm run check` 0-0 / Playwright 41.
