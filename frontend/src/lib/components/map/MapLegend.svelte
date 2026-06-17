<script lang="ts">
	// Map legend — a show/hide key to the canvas encoding, pinned to the BOTTOM of
	// the sidebar and expanding UPWARD (the sections above scroll/yield). It is
	// purely explanatory: every swatch reads from the SAME design tokens the edges
	// use, so toggling the colour-blind palette (which swaps the mass hues on the
	// .flow wrapper) recolours the legend in lock-step — the legend can't drift
	// from the canvas. Meaning is carried by TEXT beside each swatch, never colour
	// alone (the StatusIcon / edge-encoding a11y rule).
	import { m } from '$lib/paraglide/messages';

	let { open = $bindable(false), locked = false }: { open?: boolean; locked?: boolean } =
		$props();
</script>

<section class="legend" class:open data-testid="map-legend">
	<button
		type="button"
		class="legend-header"
		aria-expanded={open}
		aria-label={open ? m.map_proto_legend_close() : m.map_proto_legend_open()}
		onclick={() => !locked && (open = !open)}
		disabled={locked}
	>
		<svg
			class="chevron"
			class:open
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2.5"
			aria-hidden="true"
		>
			<path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
		</svg>
		<span class="legend-title">{m.map_proto_legend_heading()}</span>
	</button>

	{#if open}
		<div class="legend-body">
			<!-- MASS: thickness + colour. Swatches read the mass tokens so the
			     colour-blind palette swap recolours them too. -->
			<h3 class="group">{m.map_proto_legend_group_mass()}</h3>
			<ul class="rows">
				<li>
					<span class="line" style="--c: var(--mass-fresh); --w: 5px;"></span>
					<span class="label">{m.map_proto_mass_fresh()}</span>
				</li>
				<li>
					<span class="line" style="--c: var(--mass-half); --w: 3px;"></span>
					<span class="label">{m.map_proto_mass_half()}</span>
				</li>
				<li>
					<span class="line" style="--c: var(--mass-critical); --w: 2px;"></span>
					<span class="label">{m.map_proto_mass_critical()}</span>
				</li>
			</ul>

			<!-- TTL: three visual tiers (calm shows no glyph; warning a clock; critical
			     an octagon) — shape-distinct, matching ConnectionEdgeLabel. -->
			<h3 class="group">{m.map_proto_legend_group_ttl()}</h3>
			<!-- Each TTL row shows BOTH cues the edge uses: the line texture (solid /
			     dashed / dash-dot — the stroke-dasharray values from edge-encoding)
			     AND the escalating glyph. Calm is a solid line with no glyph. -->
			<ul class="rows">
				<li>
					<span class="dash" aria-hidden="true">
						<svg viewBox="0 0 28 6" preserveAspectRatio="none">
							<line x1="0" y1="3" x2="28" y2="3" stroke="var(--slate-400)" stroke-width="2.5" />
						</svg>
					</span>
					<span class="glyph" aria-hidden="true"></span>
					<span class="label">{m.map_proto_legend_ttl_calm()}</span>
				</li>
				<li>
					<span class="dash" aria-hidden="true">
						<svg viewBox="0 0 28 6" preserveAspectRatio="none">
							<line
								x1="0"
								y1="3"
								x2="28"
								y2="3"
								stroke="var(--alert-warning)"
								stroke-width="2.5"
								stroke-linecap="round"
								stroke-dasharray="14 8"
							/>
						</svg>
					</span>
					<span class="glyph" style="color: var(--alert-warning);" aria-hidden="true">
						<svg viewBox="0 0 16 16">
							<circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.5" />
							<path
								d="M8 4.5V8l2.5 1.5"
								fill="none"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					</span>
					<span class="label">{m.map_proto_legend_ttl_warning()}</span>
				</li>
				<li>
					<span class="dash" aria-hidden="true">
						<svg viewBox="0 0 28 6" preserveAspectRatio="none">
							<line
								x1="0"
								y1="3"
								x2="28"
								y2="3"
								stroke="var(--alert-danger)"
								stroke-width="2.5"
								stroke-linecap="round"
								stroke-dasharray="9 9 2 9"
							/>
						</svg>
					</span>
					<span class="glyph" style="color: var(--alert-danger);" aria-hidden="true">
						<svg viewBox="0 0 16 16">
							<path
								d="M5.2 1.5h5.6L14.5 5.2v5.6L10.8 14.5H5.2L1.5 10.8V5.2z"
								fill="none"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linejoin="round"
							/>
							<path
								d="M8 4.5v4"
								fill="none"
								stroke="currentColor"
								stroke-width="1.5"
								stroke-linecap="round"
							/>
							<circle cx="8" cy="11" r="0.9" fill="currentColor" />
						</svg>
					</span>
					<span class="label">{m.map_proto_legend_ttl_critical()}</span>
				</li>
			</ul>

			<!-- ALERT GLOW: the breathing under-stroke (casing) the edge draws on a
			     flagged connection. It is PURE TTL — mass never adds a glow (a static
			     halo on a crit-mass hole just read as broken), so the pulse is reserved
			     entirely for the time axis. Amber pulse for warning, a stronger red
			     pulse for critical; same cadence as the edge (frozen by reduced-motion). -->
			<h3 class="group">{m.map_proto_legend_group_alert()}</h3>
			<ul class="rows">
				<li>
					<span class="glow glow-amber" aria-hidden="true"></span>
					<span class="label">{m.map_proto_legend_alert_warning()}</span>
				</li>
				<li>
					<span class="glow glow-red" aria-hidden="true"></span>
					<span class="label">{m.map_proto_legend_alert_danger()}</span>
				</li>
			</ul>

			<!-- NODES: root + unconfirmed (ghost). -->
			<h3 class="group">{m.map_proto_legend_group_nodes()}</h3>
			<ul class="rows">
				<li>
					<span class="node-swatch root" aria-hidden="true"></span>
					<span class="label">{m.map_proto_root()}</span>
				</li>
				<li>
					<span class="node-swatch ghost" aria-hidden="true"></span>
					<span class="label">{m.map_proto_ghost()}</span>
				</li>
			</ul>

			<!-- OTHER: direction glyph + parallel-edge bowing. -->
			<h3 class="group">{m.map_proto_legend_group_other()}</h3>
			<ul class="rows">
				<li>
					<span class="glyph dir" aria-hidden="true">→</span>
					<span class="label">{m.map_proto_legend_direction()}</span>
				</li>
				<li>
					<span class="glyph" aria-hidden="true">
						<svg viewBox="0 0 16 16">
							<path
								d="M1 8C5 3 11 3 15 8"
								fill="none"
								stroke="var(--slate-400)"
								stroke-width="1.5"
							/>
							<path
								d="M1 8C5 13 11 13 15 8"
								fill="none"
								stroke="var(--slate-400)"
								stroke-width="1.5"
							/>
						</svg>
					</span>
					<span class="label">{m.map_proto_legend_parallel()}</span>
				</li>
			</ul>
		</div>
	{/if}
</section>

<style>
	/* Pinned footer: `flex: none` keeps it out of the scrolling sections region;
	   the body expands UPWARD because the region above is the flex child that
	   yields. A top border separates it from the scrollable sections. */
	.legend {
		flex: none;
		border-top: 1px solid var(--space-700);
		background: var(--space-900);
	}

	.legend-header {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 8px 12px;
		background: none;
		border: none;
		text-align: left;
		font-size: 10px;
		font-weight: 500;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--slate-400);
		cursor: pointer;
	}
	.legend-header:hover {
		color: var(--slate-300);
	}
	.legend-header:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}
	.chevron {
		width: 10px;
		height: 10px;
		flex: none;
		transition: transform 0.18s ease;
	}
	.chevron.open {
		transform: rotate(90deg);
	}
	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}
	.legend-title {
		flex: 1;
	}

	/* The body can grow tall; cap it and scroll internally so it never pushes the
	   canvas/sections offscreen on a short viewport. */
	.legend-body {
		padding: 2px 12px 12px;
		max-height: 40vh;
		overflow-y: auto;
	}

	.group {
		margin: 10px 0 4px;
		font-size: 9px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.07em;
		color: var(--slate-600);
	}
	.group:first-child {
		margin-top: 2px;
	}
	.rows {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 5px;
	}
	.rows li {
		display: flex;
		align-items: center;
		gap: 9px;
	}
	.label {
		font-size: 11px;
		color: var(--slate-300);
	}

	/* A short stroke sample: width + colour come from inline custom props so each
	   row reflects the real mass encoding (and re-tints with the palette swap). */
	.line {
		display: inline-block;
		width: 22px;
		height: var(--w, 2px);
		border-radius: 2px;
		background: var(--c, var(--slate-400));
		flex: none;
	}

	/* Line-texture sample: the real stroke-dasharray (solid / dashed / dash-dot)
	   the TTL tier draws on the edge. Fixed-width box so the rows align. */
	.dash {
		width: 28px;
		height: 6px;
		flex: none;
	}
	.dash svg {
		width: 100%;
		height: 100%;
		display: block;
	}

	/* Glyph cell: fixed box so labels align whether the cue is an SVG, a stroke
	   sample, or a character. */
	.glyph {
		width: 22px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
	}
	.glyph svg {
		width: 15px;
		height: 15px;
	}
	.glyph.dir {
		font-size: 16px;
		font-weight: 700;
		line-height: 1;
		color: var(--sky);
	}

	/* Alert-glow swatch: a short line segment wearing a soft, breathing halo —
	   the same casing-over-line cue the edge draws. The halo (box-shadow) swells
	   with the matching cadence; reduced-motion freezes it at a resting glow. The
	   colours read the same tokens the edge casing uses. */
	.glow {
		width: 22px;
		height: 3px;
		border-radius: 2px;
		flex: none;
	}
	.glow-amber {
		background: var(--alert-warning);
		box-shadow: 0 0 5px 1px var(--alert-warning);
		animation: legend-breathe-amber 3.4s ease-in-out infinite;
	}
	.glow-red {
		background: var(--alert-danger);
		box-shadow: 0 0 6px 2px var(--alert-danger-halo);
		animation: legend-breathe-red 2.8s ease-in-out infinite;
	}
	@keyframes legend-breathe-amber {
		0%,
		100% {
			box-shadow: 0 0 4px 0 var(--alert-warning);
		}
		50% {
			box-shadow: 0 0 8px 2px var(--alert-warning);
		}
	}
	@keyframes legend-breathe-red {
		0%,
		100% {
			box-shadow: 0 0 5px 1px var(--alert-danger-halo);
		}
		50% {
			box-shadow: 0 0 11px 4px var(--alert-danger-halo);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.glow-amber,
		.glow-red {
			animation: none;
		}
	}

	/* Node swatches mirror SystemNode's root ring + ghost dashed border. */
	.node-swatch {
		width: 18px;
		height: 12px;
		border-radius: 3px;
		background: var(--space-700);
		border: 1px solid var(--space-600);
		flex: none;
	}
	.node-swatch.root {
		border-color: var(--sky);
		box-shadow: 0 0 0 1px var(--sky);
	}
	.node-swatch.ghost {
		border-style: dashed;
		border-color: var(--slate-500);
		background: transparent;
	}
</style>
