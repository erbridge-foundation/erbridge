# status-indicators

## Purpose

Convey status throughout the UI by shape (and colour) rather than colour alone, so signals survive colour-blindness and forced-colors mode. A single shared `StatusIcon` component is the source of the severity-icon vocabulary.

## Requirements

### Requirement: Status is conveyed by shape, not colour alone

The UI SHALL convey status using a shape-distinct icon in addition to colour, so that the status is distinguishable without relying on hue. Every status icon SHALL retain visible adjacent text describing the status; the icon reinforces that text, it does not replace it.

#### Scenario: Each severity level has a distinct silhouette

- **WHEN** the three severity levels (`ok`, `warning`, `error`) are rendered side by side
- **THEN** each renders a different shape (`ok` and `error` as glyphs inside a circle, `warning` as a glyph inside a triangle) that remains distinguishable when colour is removed (greyscale / forced-colors)

#### Scenario: Colour survives theming but is not the sole signal

- **WHEN** a status icon is rendered
- **THEN** its colour is derived from `currentColor` against the existing design tokens, AND the status remains identifiable from the icon's shape and the adjacent text even if the colour is not perceivable

### Requirement: Shared StatusIcon component

A single `StatusIcon` component SHALL be the source of the status-severity icon vocabulary. It SHALL accept a `level` of `ok`, `warning`, or `error`, and an optional `tooltip` string. It SHALL render only the icon glyph — no side text, no surrounding layout — leaving callers to supply visible labels and layout in situ.

#### Scenario: Level selects the glyph

- **WHEN** `StatusIcon` is rendered with `level="ok"`, `level="warning"`, or `level="error"`
- **THEN** it renders the check-in-circle, bang-in-triangle, or cross-in-circle glyph respectively, each occupying the same optical bounding box

#### Scenario: No tooltip means decorative

- **WHEN** `StatusIcon` is rendered without a `tooltip`
- **THEN** the icon is hidden from assistive technology (`aria-hidden`) and is not focusable, so the adjacent page text is the only announced source of meaning

#### Scenario: Tooltip is exposed accessibly

- **WHEN** `StatusIcon` is rendered with a `tooltip` string
- **THEN** the icon is focusable and the tooltip text is associated with it accessibly (e.g. via `aria-describedby`) rather than only a bare `title` attribute, AND the tooltip is supplementary — the core status is still carried by the icon shape and adjacent text

### Requirement: Status sites use StatusIcon

The status indicators that previously used colour-only dots SHALL render via `StatusIcon`, with each caller mapping its domain status to a severity level and keeping its own visible text label.

#### Scenario: Connection indicator

- **WHEN** the global navigation connection indicator is shown
- **THEN** a connected state renders `StatusIcon` at `level="ok"` and a disconnected state at `level="error"`, with the existing "Connected" / "Disconnected" text retained and no pulsing animation

#### Scenario: Token status

- **WHEN** a character's token status is shown (admin Characters page or user-facing Characters page)
- **THEN** an active token renders `level="ok"`, an owner-mismatch (transferred) token renders `level="warning"`, and an expired token renders `level="error"`, alongside the existing status text

#### Scenario: Account issues roll-up

- **WHEN** the admin Characters issues cell shows expired or transferred counts
- **THEN** expired renders `level="error"` and transferred (owner mismatch) renders `level="warning"`, alongside the existing count text
