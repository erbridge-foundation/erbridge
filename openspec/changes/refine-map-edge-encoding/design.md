## Context

Iterates the `map-canvas-prototype` capability (archived change
`2026-06-17-build-map-canvas-prototype`). The prototype already renders connections as
floating bezier edges with a midpoint label and endpoint sig pills. This change enriches the
*edge encoding* and corrects two model facts found while building it. Retroactive: the code
exists in the working tree on `feat/map-canvas-prototype`.

Source of the encoding: a handoff spec ("Wormhole Map — Edge Encoding Spec") plus a series
of in-session decisions captured in the explore ledger
(`openspec/changes/explore-map-canvas-prototype/design.md`).

## Goals / Non-Goals

**Goals:**
- Read mass and TTL as two INDEPENDENT variables on the line, plus a derived alert.
- Keep every cue legible without colour or motion (greyscale, forced-colors, reduced-motion,
  colour-blind) — meaning is carried by thickness + dash + shape + text.
- One pure, unit-testable resolver that turns `(mass, ttl_remaining_min)` into every channel.
- A runtime colour-blind palette toggle for visual comparison.

**Non-Goals:**
- Real TTL data / a clock — the prototype carries literal `ttl_remaining_min` per fixture
  connection; deriving it from `opened_at + max_stable_time` is backend (Track 2) work.
- The "propagation group / direction-on-the-pill" rework (separate future change — see the
  explore ledger). This change still keeps the endpoint arrowhead for KNOWN direction.
- Endpoint-spread / ELK-LR layout.

## Decisions

- **Channel ownership (separate reading from alerting).** Mass → thickness + colour; TTL →
  dash + a single glyph; ALERT (derived) → casing + label badge. A thin red line is its own
  alarm, so colour stays mass-owned; "grabbing the eye" is a separate derived layer.
- **Mass widths fresh 5 / half 3 / critical 2px**, critical floored at 2 so the imminent
  dash-dot doesn't collapse to solid. The edge-thickness slider is a MULTIPLIER
  (`width * slider/2`), preserving the ordering at every value rather than setting an
  absolute width (which would clobber the mass cue).
- **Four TTL states, three visual tiers.** Model keeps `stable|lt4h|lt1h|imminent`
  (`ttlState(min)`); the map collapses via `ttlVisual()` to `calm|warning|critical`, with
  **`lt1h` and `imminent` identical** (critical/red). Rationale: < 1 h is the actionable
  "act now" signal; imminent is "too late" and says nothing new. The four-state enum is
  retained so the label's accessible text stays precise ("less than 1 hour" vs "closure
  imminent").
- **Alert glow is PURE TTL.** Mass-critical does NOT add a casing — the thin red line
  already conveys it, and a static (non-pulsing) crit-mass halo read as "broken". Reserving
  the breathing halo for the time axis keeps motion rare and meaningful.
- **Richer halo red.** Casing uses `--alert-danger-halo: #ff3b30` (vivid alarm), distinct
  from the colour-blind-safe vermillion `--alert-danger: #d55e00` used for line/glyph/badge,
  because the latter desaturates to a dim ember at the halo's low opacity.
- **Colour-blind palette = CSS-only swap.** A `data-edge-palette` attribute on the `.flow`
  wrapper re-points only the three `--mass-*` tokens (Okabe-Ito blue/orange/vermillion);
  thickness, dash, glyph, motion, alert are identical — so the toggle is a one-line change
  and the resolver never branches on palette.
- **Undetermined direction → nothing.** Removed the neutral mid-edge marker; a missing
  arrowhead already reads as "direction not yet known". (Pre-empts the propagation-group
  rework, which removes the arrowhead entirely.)
- **`SystemStatic.code` → `wh_type`.** The field now holds the real wormhole-type code (e.g.
  `C729`), kept for the later signature-scanning work; the node/sidebar display only the
  static's destination class (HS/LS/C5…) since the type isn't user-facing yet.
- **Pochven = `SystemClass 'P'`.** Modelled as its own space type (not NS/LS): null-ish
  security but a distinct Triglavian region with its own access/connectivity. Distinct
  `--pochven` token across all three palette blocks.

## Risks / Trade-offs

- **Arrow-tangent vs. spread** is deferred: this change keeps endpoint arrowheads, which the
  propagation-group rework will revisit. Acceptable because that rework is separately
  designed and scheduled with ELK-LR.
- **`--alert-warning`/`--alert-danger-halo` are not overridden in high-contrast**; they only
  tint decoration (shape/dash/text carry meaning), so flattening them loses no information.
- **`eol: boolean` is retained on `Connection`** as a now-derived/decorative flag during the
  prototype to avoid a wider refactor; the encoding reads TTL, not `eol`.
- **Static `wh_type` values in the fixture** are plausible real codes, not authoritative —
  the real catalog supplies them later. Low blast radius (sandbox fixture).
