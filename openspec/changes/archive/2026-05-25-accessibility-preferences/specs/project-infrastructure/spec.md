## MODIFIED Requirements

### Requirement: Frontend applies the E-R Bridge design system
The frontend SHALL implement the visual design language established in the wireframe. All pages and components SHALL use this system consistently.

**Typography**
The default typeface is JetBrains Mono (Google Fonts), applied globally via a `--font-ui` custom property (defaulting to `"JetBrains Mono", ui-monospace, monospace`) that `<body>` and everything inheriting from it use. The indirection exists so the `dyslexia_font` accessibility preference can swap the whole UI to an alternative typeface (Atkinson Hyperlegible) by overriding `--font-ui` on `<html>` (see the `accessibility-preferences` capability). The `<html>` element's `font-size` defaults to `100%` (picking up the browser/OS default, typically 16px) but is **user-controllable** via the `text_size` accessibility preference, which overrides `html { font-size }`. The `<body>` SHALL be set to `font-size: 0.875rem` (≈14px at the default). **All typography rules across the design system SHALL be expressed in `rem`, not `px`**, so the UI scales both when a visitor changes their browser font-size or zooms AND when they change the `text_size` preference. Spacing (padding, margin, gap, border-radius, avatar/icon dimensions, border widths) is exempt and SHALL remain in `px`, since those are visual-layout values that must not grow with text-size preferences.

**Motion**
All animations and transitions SHALL be gated on the reduce-motion preference (which defaults to the OS `prefers-reduced-motion` setting), so motion in the design system — including the pulsing status dot — does not bypass the accessibility preference (see the `accessibility-preferences` capability).

**Colour tokens** — defined as CSS custom properties on `:root`:

| Token | Value | Role |
|---|---|---|
| `--space-950` | `#05080f` | Page / canvas background |
| `--space-900` | `#080d1a` | Surface: nav, sidebar, panels |
| `--space-800` | `#0d1526` | Raised surface: inputs, nodes |
| `--space-700` | `#152238` | Borders, dividers |
| `--space-600` | `#1e3352` | Subtle borders, input outlines |
| `--slate-100` | `#f1f5f9` | Primary text |
| `--slate-200` | `#e2e8f0` | Hover text |
| `--slate-300` | `#cbd5e1` | Secondary text |
| `--slate-400` | `#94a3b8` | Muted text, nav links |
| `--slate-500` | `#64748b` | Placeholder, icon resting |
| `--slate-600` | `#475569` | Disabled / count labels |
| `--emerald` | `#10b981` | Online status, positive actions |
| `--amber` | `#f59e0b` | Warning, history mode |
| `--red` | `#ef4444` | Destructive, critical mass |
| `--sky` | `#38bdf8` | Brand accent (logo, active tab indicator) |
| `--violet` | `#a78bfa` | Named-root pill, code |

**Global nav bar** (`height: 48px`, `background: var(--space-900)`, `border-bottom: 1px solid var(--space-700)`):
- Left: brand logo SVG in `--sky` + wordmark `E-R BRIDGE` in 12px/600 weight with `letter-spacing: 0.2em`, separated from nav links by a `1px solid var(--space-700)` rule
- Nav links: 11px, `color: var(--slate-400)`; on hover/active: `color: var(--slate-200)`, `background: var(--space-700)`, `border-radius: 4px`; height 28px, padding `0 12px`
- Right side: pulsing emerald status dot + "connected" label; find-system input (`width: 180px`, `background: var(--space-800)`, `border: 1px solid var(--space-600)`); icon-only logout button

**Left sidebar** (`width: 288px`, collapsible to 40px icon rail):
- `background: var(--space-900)`, `border-right: 1px solid var(--space-700)`
- Collapsible via a 24px circular toggle button that overflows the sidebar's right edge (`right: -12px`); icon rotates 180° when collapsed
- Collapsed state: hides all text, section bodies, and counts — shows only section icon in a centred rail
- Section headers: 10px uppercase `letter-spacing: 0.08em`, `color: var(--slate-400)`; include a chevron (rotates 90° when open), a title, an optional count in `--slate-600`, and an optional action icon button (`color: var(--slate-500)`, hover `color: var(--emerald)`)
- Sections separated by `border-bottom: 1px solid var(--space-700)`
- Sidebar scrolls vertically with `scrollbar-width: thin; scrollbar-color: var(--space-600) transparent`

**Login / unauthenticated page** — applies the same shell: full-height dark background (`--space-950`). if a user is not authenticated, they SHALL redirect to this /login page. the main area SHALL centre the login call-to-action.

#### Scenario: Global nav renders correctly on all pages
- **WHEN** any page is loaded
- **THEN** the nav bar is 48px tall with `--space-900` background, brand logo in `--sky`, and nav links styled per the design system

#### Scenario: Login page uses the design system
- **WHEN** an unauthenticated user loads `/login`
- **THEN** the page background is `--space-950`, the nav bar is NOT present

#### Scenario: Sidebar collapses to icon rail
- **WHEN** the sidebar toggle is clicked on the authenticated map view
- **THEN** the sidebar width transitions to 40px, text is hidden, and only section icons remain visible

#### Scenario: Text size preference scales typography
- **WHEN** a user sets the `text_size` accessibility preference away from its default
- **THEN** `html { font-size }` changes and all `rem`-based typography scales proportionally, while `px` spacing values are unaffected
