# Tasks — refine-map-edge-encoding

Retroactive: the work is already implemented on `feat/map-canvas-prototype` (uncommitted
working tree at proposal time). Tasks are recorded complete and reflect what shipped.

## 1. Model (types + fixture)

- [x] 1.1 Add the TTL model to `lib/map/types.ts`: `MaxStableMin` + `MAX_STABLE_MIN`, the
  four-state `TtlState` + `ttlState(min)`, the three-tier `TtlVisual` + `ttlVisual(state)`.
- [x] 1.2 Add `ttl_remaining_min` to `Connection`; retain `eol` as a now-derived/decorative
  flag.
- [x] 1.3 Rename `SystemStatic.code` → `wh_type` (real wormhole-type code, kept for later
  signature work).
- [x] 1.4 Add `'P'` (Pochven) to `SystemClass`.
- [x] 1.5 Update the map fixture: `ttl_remaining_min` on every connection (covering all
  mass × TTL combos incl. fresh+imminent), `wh_type` statics, and a Pochven system
  (`Krirald`) reached from J100004.

## 2. Edge-encoding resolver

- [x] 2.1 Add `lib/map/edge-encoding.ts`: the pure resolver
  `resolveEdgeEncoding(mass, ttlRemainingMin, palette?)` → mass (width+colourVar), ttl
  (dash+glyph+tint), alert (casing+breathe), `ttlBucket`, `ttlVisual`.
- [x] 2.2 Mass widths fresh 5 / half 3 / critical 2 (floored); alert keyed off `ttlVisual`
  ONLY (mass contributes no casing); `lt1h` and `imminent` resolve identically.

## 3. Render (components + tokens)

- [x] 3.1 `ConnectionEdge.svelte`: drive stroke width/colour/dash from the resolver; treat
  the thickness slider as a multiplier; render the breathing casing as a second `<BaseEdge>`
  under-stroke; remove the neutral undetermined-direction mid-edge marker.
- [x] 3.2 `ConnectionEdgeLabel.svelte`: shape-distinct inline-SVG TTL glyphs with accessible
  text; border keyed off the alert level.
- [x] 3.3 `MapCanvas.svelte`: pass `ttl_remaining_min` into edge data; add the colour-blind
  palette state + `data-edge-palette` attribute on the `.flow` wrapper; arrowhead follows
  mass colour.
- [x] 3.4 `MapSidebar.svelte`: colour-blind palette toggle; statics show destination class.
- [x] 3.5 `SystemNode.svelte`: statics show destination class (not the type code); add
  Pochven to `classColour`.
- [x] 3.6 `app.css`: `--alert-warning`, `--alert-danger`, `--alert-danger-halo`,
  `--text-secondary`, `--pochven` (incl. both high-contrast blocks), and the
  `[data-edge-palette="colourblind"]` mass-hue swap; update the spec's standard mass hues.
- [x] 3.7 i18n: TTL state labels + colour-blind-palette label added to `messages/{en,de,fr}.json`.

## 4. Tests

- [x] 4.1 Unit tests for the resolver (mass widths, TTL bucketing, three-tier collapse,
  pure-TTL alert, lt1h≡imminent) and the label encoding (glyph + sr-text, dest-class static).
- [x] 4.2 Update e2e `map-proto.spec.ts`: assert TTL text + breathing casing, palette-toggle
  attribute swap; drop the stale `⚠` assertion.

## 5. Docs

- [x] 5.1 Record the in-session decisions (three-tier collapse, pure-TTL glow, propagation
  group) in the explore ledger `explore-map-canvas-prototype/design.md`.
- [x] 5.2 Update `openspec/AGENTS.md`: add `edge-encoding` to the `map/` module note and
  the TTL/Pochven additions to the `types` note (the module enumeration would otherwise go
  stale).

## 6. Verification (run from `frontend/`)

- [x] 6.1 `pnpm test` — Vitest unit/component (396 passing).
- [x] 6.2 `pnpm run check` — svelte-check + paraglide compile (0 errors / 0 warnings).
- [x] 6.3 `pnpm run test:e2e` — Playwright e2e (41 passing).
