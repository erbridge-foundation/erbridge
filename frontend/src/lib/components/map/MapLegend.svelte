<script lang="ts">
	// Map legend — a show/hide key to the canvas encoding, pinned to the BOTTOM of
	// the sidebar and expanding UPWARD (the sections above scroll/yield). It is
	// purely explanatory: every swatch reads from the SAME design tokens the edges
	// use, so toggling the colour-blind palette (which swaps the mass hues on the
	// .flow wrapper) recolours the legend in lock-step — the legend can't drift
	// from the canvas. Meaning is carried by TEXT beside each swatch, never colour
	// alone (the StatusIcon / edge-encoding a11y rule).
	import { m } from '$lib/paraglide/messages';
	import { INTEL_FLAGS, type SystemFlag } from '$lib/map/types';

	let { open = $bindable(false), locked = false }: { open?: boolean; locked?: boolean } =
		$props();

	// Mirror SystemNode's intel-flag glyph + colour + label so the legend swatches read
	// the SAME tokens/glyphs the nodes draw and can't drift from the canvas.
	const flagGlyph: Record<SystemFlag, string> = {
		target: '◎',
		warning: '⚠',
		friendly: '✚',
		'looking-for': '⌕'
	};
	const flagColour: Record<SystemFlag, string> = {
		target: 'var(--violet)',
		warning: 'var(--alert-warning)',
		friendly: 'var(--emerald)',
		'looking-for': 'var(--sky)'
	};
	const flagLabel: Record<SystemFlag, string> = {
		target: m.map_proto_flag_target(),
		warning: m.map_proto_flag_warning(),
		friendly: m.map_proto_flag_friendly(),
		'looking-for': m.map_proto_flag_looking_for()
	};
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

			<!-- TTL: carried PURELY by the breathing background casing now (the dashed-
			     line texture was dropped). Only the two tiers that SHOW a cue are listed —
			     warning = amber pulse; critical = a stronger, LARGER red pulse. STABLE is
			     the implicit default (no glow), so it gets no row: a grey-line swatch just
			     had people hunting for a line that isn't drawn. The pulse cadence + sizes
			     match the edge; reduced-motion freezes each at its MAX (so warning vs
			     critical stay tellable apart by size). -->
			<h3 class="group">{m.map_proto_legend_group_ttl()}</h3>
			<ul class="rows">
				<li>
					<span class="glow-cell" aria-hidden="true"><span class="glow glow-amber"></span></span>
					<span class="label">{m.map_proto_legend_ttl_warning()}</span>
				</li>
				<li>
					<span class="glow-cell" aria-hidden="true"><span class="glow glow-red"></span></span>
					<span class="label">{m.map_proto_legend_ttl_critical()}</span>
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
				<li>
					<span class="node-swatch dangling" aria-hidden="true">?</span>
					<span class="label">{m.map_proto_dangling()}</span>
				</li>
			</ul>

			<!-- SYSTEM FLAGS: the intel markers a system can carry (composable). Each row's
			     swatch is the SAME glyph chip the node draws, reading the same colour token. -->
			<h3 class="group">{m.map_proto_legend_group_flags()}</h3>
			<ul class="rows">
				{#each INTEL_FLAGS as f (f)}
					<li>
						<span class="badge flag" style:--badge-colour={flagColour[f]} aria-hidden="true"
							>{flagGlyph[f]}</span
						>
						<span class="label">{flagLabel[f]}</span>
					</li>
				{/each}
			</ul>

			<!-- OTHER: the direction glyph. -->
			<h3 class="group">{m.map_proto_legend_group_other()}</h3>
			<ul class="rows">
				<li>
					<span class="glyph dir" aria-hidden="true">→</span>
					<span class="label">{m.map_proto_legend_direction()}</span>
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

	/* Glyph cell: fixed box so labels align (the direction arrow is a character). */
	.glyph {
		width: 22px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
	}
	.glyph.dir {
		font-size: 16px;
		font-weight: 700;
		line-height: 1;
		color: var(--sky);
	}

	/* TTL swatch: the casing, drawn the SAME way the edge draws it — a translucent
	   BAND of a definite width (a scaled-down stroke), NOT a blurred box-shadow. The
	   edge casing is an SVG stroke (hard-edged, fades by opacity, no Gaussian blur),
	   so the swatch is a solid translucent rectangle to match — same colour tokens +
	   opacities as edge-encoding (warning amber @0.3, critical halo-red @0.5). Heights
	   are the edge widths (16 / 26) scaled to swatch size, so critical reads LARGER.
	   No line core: the real line keeps its mass colour, so the legend shows only the
	   cue that changes. Breathes height+opacity from the PEAK so a reduced-motion
	   freeze lands on the max (matching the edge). */
	.glow-cell {
		width: 22px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
	}
	.glow {
		width: 22px;
		border-radius: 2px;
		flex: none;
	}
	.glow-amber {
		height: 6px;
		background: var(--alert-warning);
		opacity: 0.3;
		animation: legend-breathe-amber 3.4s ease-in-out infinite;
	}
	.glow-red {
		height: 10px;
		background: var(--alert-danger-halo);
		opacity: 0.5;
		animation: legend-breathe-red 2.8s ease-in-out infinite;
	}
	/* PEAK at 0%/100% (matches the resting height/opacity), trough at 50% → freeze =
	   max, mirroring the edge casing's width pulse. */
	@keyframes legend-breathe-amber {
		0%,
		100% {
			height: 6px;
			opacity: 0.3;
		}
		50% {
			height: 4px;
			opacity: 0.16;
		}
	}
	@keyframes legend-breathe-red {
		0%,
		100% {
			height: 10px;
			opacity: 0.5;
		}
		50% {
			height: 5px;
			opacity: 0.16;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.glow-amber,
		.glow-red {
			animation: none;
		}
	}

	/* Intel-flag swatch: the same bordered glyph chip SystemNode draws. The colour
	   decorates the border + glyph; the glyph shape + the label text carry the meaning
	   (so it survives greyscale / forced-colors, matching the node). */
	.badge.flag {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 16px;
		flex: none;
		border-radius: 3px;
		border: 1px solid var(--badge-colour);
		color: var(--badge-colour);
		font-size: 11px;
		font-weight: 700;
		line-height: 1;
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
	/* Dangling stub: dotted + a faint `?` glyph, matching the SystemNode stub. */
	.node-swatch.dangling {
		display: flex;
		align-items: center;
		justify-content: center;
		border-style: dotted;
		border-color: var(--slate-600);
		background: transparent;
		font-size: 9px;
		font-weight: 700;
		line-height: 1;
		color: var(--slate-400);
	}
</style>
