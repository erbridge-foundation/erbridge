<script lang="ts">
	// Shared severity icon: shape-distinct, currentColor-driven inline SVG for a
	// three-level scale. Shapes stay distinguishable in greyscale / forced-colors
	// (round vs. pointed, check vs. cross) so status never relies on hue alone.
	//
	// Glyph-only by design: callers own the side text and layout. Each caller maps
	// its domain status to a `level` and keeps its own visible label in situ.

	let { level, tooltip }: { level: 'ok' | 'warning' | 'error'; tooltip?: string } = $props();

	// currentColor drives the glyph; map each level to its design token here so the
	// wrapper carries the colour and the SVG inherits it.
	const colour: Record<typeof level, string> = {
		ok: 'var(--emerald)',
		warning: 'var(--amber)',
		error: 'var(--red)'
	};

	// Stable id for aria-describedby when a tooltip is present.
	const tooltipId = `status-icon-tip-${Math.random().toString(36).slice(2)}`;
</script>

{#snippet glyph()}
	{#if level === 'ok'}
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<circle cx="12" cy="12" r="10" fill="currentColor" />
			<path
				class="mark"
				d="M7.5 12.5l3 3 6-6.5"
				fill="none"
				stroke-width="2.25"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
	{:else if level === 'error'}
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<circle cx="12" cy="12" r="10" fill="currentColor" />
			<path
				class="mark"
				d="M8.5 8.5l7 7M15.5 8.5l-7 7"
				fill="none"
				stroke-width="2.25"
				stroke-linecap="round"
				stroke-linejoin="round"
			/>
		</svg>
	{:else}
		<svg viewBox="0 0 24 24" aria-hidden="true">
			<path
				d="M12 2.5L22.5 21H1.5L12 2.5z"
				fill="currentColor"
				stroke="currentColor"
				stroke-width="1.5"
				stroke-linejoin="round"
			/>
			<path class="mark" d="M12 9.5v5" fill="none" stroke-width="2.25" stroke-linecap="round" />
			<circle class="mark-fill" cx="12" cy="17.5" r="1.2" />
		</svg>
	{/if}
{/snippet}

<!--
	Two modes:
	- no tooltip → decorative: a plain span, aria-hidden, non-focusable; adjacent
	  page text is the only announced source of meaning.
	- with tooltip → a real <button> (naturally focusable + keyboard-operable),
	  named via aria-label, with the supplementary tooltip associated via
	  aria-describedby (never a bare title).
-->
{#if tooltip}
	<button
		type="button"
		class="status-icon"
		data-level={level}
		style:color={colour[level]}
		aria-label={tooltip}
		aria-describedby={tooltipId}
	>
		{@render glyph()}
		<span class="tooltip" id={tooltipId} role="tooltip">{tooltip}</span>
	</button>
{:else}
	<span class="status-icon" data-level={level} style:color={colour[level]} aria-hidden="true">
		{@render glyph()}
	</span>
{/if}

<style>
	.status-icon {
		display: inline-flex;
		position: relative;
		width: 1em;
		height: 1em;
		flex-shrink: 0;
		/* Optical sizing: the SVG fills the box; callers size via font-size. */
		line-height: 1;
		/* Shape already survives forced-colors (it's geometry, not colour), but
		   keep the semantic token colour too so the redundant hue signal isn't
		   flattened to a single system colour. */
		forced-color-adjust: none;
	}
	/* When the icon is interactive (tooltip mode), strip the native button chrome
	   so it reads as a bare glyph. */
	button.status-icon {
		padding: 0;
		border: 0;
		background: transparent;
		color: inherit;
		cursor: default;
	}
	.status-icon svg {
		width: 100%;
		height: 100%;
		display: block;
	}
	/* The shape fills in the level colour (currentColor); the glyph is punched out
	   in a dark contrast colour that reads on emerald, amber, and red alike. */
	.status-icon :global(.mark) {
		stroke: var(--space-950);
	}
	.status-icon :global(.mark-fill) {
		fill: var(--space-950);
	}
	.status-icon:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
		border-radius: 50%;
	}

	.tooltip {
		position: absolute;
		bottom: calc(100% + 6px);
		left: 50%;
		transform: translateX(-50%);
		z-index: 10;
		padding: 4px 8px;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font-size: 0.6875rem;
		line-height: 1.3;
		white-space: nowrap;
		pointer-events: none;
		opacity: 0;
		visibility: hidden;
		transition: opacity 0.1s ease;
	}
	.status-icon:hover .tooltip,
	.status-icon:focus-visible .tooltip {
		opacity: 1;
		visibility: visible;
	}
	@media (prefers-reduced-motion: reduce) {
		.tooltip {
			transition: none;
		}
	}
</style>
