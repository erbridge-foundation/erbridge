## Context

Status is signalled in four places by hand-rolled, colour-only dots (`<span class="dot">` with a `background: var(--red|--amber|--emerald)`):

- `GlobalNav.svelte` — SSE connection (emerald/pulsing vs red).
- `admin/characters/+page.svelte` — per-character token status, and an issues roll-up cell.
- `characters/+page.svelte` — user-facing per-character token status (its `.token-status` CSS is copy-pasted from the admin page).

All dots already have visible text beside them and are `aria-hidden`, so the screen-reader/WCAG-text story is fine. The gap is the *visual* channel: colour-blind users can't separate the hues at a glance, and forced-colors mode (recently introduced) flattens the dot backgrounds to one system colour.

The frontend convention is hand-authored inline `<svg>` with `currentColor` (see `GlobalNav` brand mark and `UserChip`) — there is no icon library, no `.svg` asset files, and no existing `Icon` component.

## Goals / Non-Goals

**Goals:**

- Convey status by shape *and* colour (redundant encoding) so it survives colour-blindness and forced-colors.
- A single shared `StatusIcon` component as the source of the severity vocabulary, removing the duplicated dot CSS.
- Keep the component a pure glyph: callers own side text and layout.
- Support an optional, accessibly-exposed tooltip for supplementary detail.
- Design the level vocabulary so a future "EVE down" warning is a caller change, not a component change.

**Non-Goals:**

- Computing an "EVE down" / degraded connection state (future GlobalNav change; it will just request `level="warning"`).
- A `WormholeEffectIcon` (Pulsar / Black Hole / Magnetar / …) — that is a domain/category marker, not a severity icon, and is data-blocked on catalog effects.
- Touching text-pill badges (`badge-expired`, `badge-main`) — their meaning is already in text, not colour.
- Any backend, API, or dependency change.

## Decisions

**Inline SVG, not raster images.** The request was "replace colour dots with images." Raster images don't recolour with `currentColor`, don't adapt to forced-colors, don't scale crisply, and add network requests. Inline SVG matches the existing convention and gives full shape + theming control. "Images" resolves to inline SVG glyphs.

**Three-level semantic severity (`ok` / `warning` / `error`), not domain names.** A semantic scale is reusable by any caller (connection, tokens, future EVE-down) and keeps the component ignorant of domain meaning. Callers map their domain status → level. Alternative considered: domain names (`active`/`owner_mismatch`/`expired`) — rejected as it would lock the component to tokens and need new states for every new consumer.

**Shape language with a shared bounding box.**

```
   ok           warning        error
   ┌───────┐    ┌───────┐     ┌───────┐
   │  ╱─╲  │    │   ▲   │     │  ╱─╲  │
   │ ( ✓ ) │    │  ▲!▲  │     │ ( ✕ ) │
   │  ╲─╱  │    │ ▲▲▲▲▲ │     │  ╲─╱  │
   └───────┘    └───────┘     └───────┘
   green ✓-circle  amber !-triangle  red ✕-circle
```

`ok` and `error` are a check / cross inside a **circle**; `warning` is a bang inside a **triangle**. The circle's diameter equals the triangle's bounding box (same max width and height) so all three align optically in a row. Greyscale-distinct on two axes: round-vs-pointed separates warning from the rest; check-vs-cross separates ok from error. Colour reinforces but is not required. Alternatives considered: distinct-shape CSS clip-paths (harder to control, weaker forced-colors story) and Unicode glyph characters (font-rendering-dependent) — both rejected for inline SVG's precision.

**Component is glyph-only; pages own text + layout.** `StatusIcon` renders just the `<svg>`. The existing `<span class="token-status">…<span>label</span></span>` wrappers stay on the pages and continue to own the gap/flex/colour-of-text. This keeps the component trivially reusable and avoids it making layout decisions per site.

**Two tooltip modes.** Absent tooltip → `aria-hidden`, non-focusable, decorative (today's behaviour; the side text is the announced source). Present tooltip → focusable with the tooltip associated via `aria-describedby` (or an equivalent accessible mechanism), never a bare `title` (invisible to keyboard, unreliable for SR, absent on touch). The tooltip is always supplementary — meaning is still carried by shape + side text.

**Drop the connection-dot pulse.** The current nav dot pulses to signal live-ness. Converting it to a static `StatusIcon` removes the pulse. Accepted per the directive "no pulsing"; the connection state remains clear from shape + text.

## Risks / Trade-offs

- **Loss of the live-ness pulse on the nav indicator** → Accepted; status clarity is unchanged (shape + text). Can be revisited if users miss the heartbeat.
- **Tooltip accessibility is easy to get wrong** (defaulting to bare `title`) → The spec mandates `aria-describedby`/accessible exposure and focusability; covered by a component test.
- **Mapping drift between sites** (each caller hand-maps domain → level) → Small, closed mappings (3 values each); kept in the call site next to the existing status logic, with e2e coverage of the rendered output.
- **Forced-colors regression elsewhere** → Out of scope sites are text pills, unaffected; the converted sites improve under forced-colors.

## Migration Plan

Pure additive frontend change, deployable in one step:

1. Add `StatusIcon.svelte` + `StatusIcon.test.ts`.
2. Convert the three call-site files; delete the now-dead `.dot` / `.token-status` colour CSS.
3. Update `openspec/AGENTS.md` component tree.
4. Verify from `frontend/`: `pnpm test`, `pnpm run check`, `pnpm run test:e2e`.

Rollback: revert the change; no data, schema, or API state is touched.

## Open Questions

- None blocking. The exact accessible-tooltip mechanism (`aria-describedby` vs the native popover/`title` combo) is left to implementation, constrained by the spec's "not a bare title" requirement.
