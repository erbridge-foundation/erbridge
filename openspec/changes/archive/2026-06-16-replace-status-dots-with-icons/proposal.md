## Why

Status across the UI is signalled by colour-only dots (red/amber/emerald) for token state, account issues, and the nav connection indicator. Colour-blind users cannot distinguish these at a glance, and in forced-colors mode the background-coloured dots flatten to a single system colour, erasing the distinction entirely. The dots already sit beside visible text (so screen-reader/WCAG-text coverage is fine), but the *visual* signal carries meaning in only one channel. Adding **shape** as a redundant channel fixes the at-a-glance gap.

## What Changes

- Add a shared `StatusIcon` component that renders a shape-distinct, currentColor-driven inline SVG for a three-level severity scale (`ok` / `warning` / `error`). Shapes are greyscale-distinguishable: check-in-circle (ok), bang-in-triangle (warning), cross-in-circle (error), all sharing one optical bounding box.
- The component supports an optional `tooltip` string. With no tooltip the icon is decorative (`aria-hidden`) and adjacent page text carries meaning, as today. With a tooltip the icon becomes focusable and the tooltip is exposed accessibly (not a bare `title`); the tooltip is supplementary detail only.
- Replace the colour-only dots at every status site with `StatusIcon`, callers mapping their domain status to a severity level and keeping their own in-situ side text:
  - GlobalNav connection indicator: connected → `ok`, disconnected → `error` (**removes** the existing pulse animation).
  - Admin Characters token-status and issues dots: active → `ok`, owner_mismatch → `warning`, expired → `error`.
  - User-facing Characters token-status dot: same mapping (also removes the token-status CSS currently copy-pasted from the admin page).
- Out of scope but designed-for: a future "EVE down" state is a caller decision (GlobalNav requests `level=warning` + tooltip) needing no `StatusIcon` change; a future `WormholeEffectIcon` is a separate domain-marker component, not a severity icon.

## Capabilities

### New Capabilities
- `status-indicators`: A shared severity-icon component and the requirement that status throughout the UI is conveyed by shape (and colour) rather than colour alone.

### Modified Capabilities
<!-- No existing spec's requirements change; the affected pages gain a presentation requirement captured under the new capability. -->

## Impact

- **New:** `frontend/src/lib/components/StatusIcon.svelte` + co-located `StatusIcon.test.ts`.
- **Modified:** `frontend/src/lib/components/GlobalNav.svelte`, `frontend/src/routes/admin/characters/+page.svelte`, `frontend/src/routes/characters/+page.svelte` — swap hand-rolled colour dots for `StatusIcon`; delete the now-dead dot CSS.
- **Docs:** `openspec/AGENTS.md` component tree gains `StatusIcon`.
- No backend, API, or dependency changes. No new libraries (inline SVG matches the existing GlobalNav/UserChip convention).
- Verification (from `frontend/`): `pnpm test`, `pnpm run check`, `pnpm run test:e2e`.
