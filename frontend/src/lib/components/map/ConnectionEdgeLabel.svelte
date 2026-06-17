<script lang="ts">
	// The encoding-bearing label of a connection edge, factored OUT of
	// ConnectionEdge.svelte so it can be unit-tested without SvelteFlow's edge
	// pipeline (which needs measured node dimensions jsdom can't provide). It owns
	// the COLOUR-INDEPENDENT half of the encoding: the wormhole type, a mass TEXT
	// cue, the single TTL glyph (shape-distinct inline SVG, escalating clock →
	// triangle → octagon), and the derived alert BADGE — none of which rely on hue
	// or motion (greyscale + reduced-motion both stay legible — spec §1/§6/§7).
	import { m } from '$lib/paraglide/messages';
	import type { Mass, TtlState } from '$lib/map/types';
	import type { TtlGlyph } from '$lib/map/edge-encoding';

	let {
		wh_type,
		mass,
		ttlBucket,
		glyph,
		glyphColourVar,
		alertLevel,
		showMass = true,
		showWhType = true
	}: {
		wh_type: string;
		mass: Mass;
		ttlBucket: TtlState;
		glyph: TtlGlyph;
		glyphColourVar: string;
		alertLevel: 'none' | 'warning' | 'danger';
		showMass?: boolean;
		showWhType?: boolean;
	} = $props();

	const massColour: Record<Mass, string> = {
		fresh: 'var(--mass-fresh)',
		half: 'var(--mass-half)',
		critical: 'var(--mass-critical)'
	};
	const massText: Record<Mass, string> = {
		fresh: m.map_proto_mass_fresh(),
		half: m.map_proto_mass_half(),
		critical: m.map_proto_mass_critical()
	};

	// Human-readable TTL label, doubling as the glyph's accessible name (so the
	// time state survives loss of colour AND of the glyph shape).
	const ttlText: Record<TtlState, string> = {
		stable: m.map_proto_ttl_stable(),
		lt4h: m.map_proto_ttl_lt4h(),
		lt1h: m.map_proto_ttl_lt1h(),
		imminent: m.map_proto_ttl_imminent()
	};

	// When the alert fires, the glyph is rendered as a FILLED badge (white glyph on
	// a coloured disc) so it pops regardless of line weight; otherwise it's a plain
	// tinted outline glyph. Stable shows no glyph at all.
	const badged = $derived(alertLevel !== 'none' && glyph !== 'none');
</script>

<div class="edge-label" data-alert={alertLevel} data-mass={mass} data-ttl={ttlBucket}>
	{#if showWhType}
		<span class="wh-type">{wh_type}</span>
	{/if}
	{#if showMass}
		<!-- Mass as TEXT — survives forced-colors and colourblindness. -->
		<span class="mass" style:--mass-colour={massColour[mass]}>{massText[mass]}</span>
	{/if}
	{#if glyph !== 'none'}
		<!-- The single TTL glyph. Shape distinguishes the state without colour;
		     badge variant (filled disc) is used when the alert fires. The text label
		     is the accessible name so the state survives loss of shape too. -->
		<span
			class="ttl-glyph"
			class:badged
			data-level={alertLevel}
			style:--glyph-colour={glyphColourVar}
			title={ttlText[ttlBucket]}
		>
			{#if glyph === 'clock'}
				<svg viewBox="0 0 16 16" aria-hidden="true">
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
			{:else if glyph === 'triangle'}
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path
						d="M8 2 14.5 13.5H1.5Z"
						fill={badged ? 'currentColor' : 'none'}
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linejoin="round"
					/>
					<path
						d="M8 6.5V9.5"
						fill="none"
						stroke={badged ? 'var(--space-950)' : 'currentColor'}
						stroke-width="1.5"
						stroke-linecap="round"
					/>
					<circle cx="8" cy="11.5" r="0.9" fill={badged ? 'var(--space-950)' : 'currentColor'} />
				</svg>
			{:else if glyph === 'octagon'}
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path
						d="M5.2 1.5h5.6L14.5 5.2v5.6L10.8 14.5H5.2L1.5 10.8V5.2Z"
						fill={badged ? 'currentColor' : 'none'}
						stroke="currentColor"
						stroke-width="1.5"
						stroke-linejoin="round"
					/>
					<path
						d="M8 5V8.5"
						fill="none"
						stroke={badged ? 'var(--space-950)' : 'currentColor'}
						stroke-width="1.5"
						stroke-linecap="round"
					/>
					<circle cx="8" cy="11" r="0.9" fill={badged ? 'var(--space-950)' : 'currentColor'} />
				</svg>
			{/if}
			<span class="sr-only">{ttlText[ttlBucket]}</span>
		</span>
	{/if}
</div>

<style>
	.edge-label {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.1rem 0.35rem;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		font-family: var(--font-ui);
		font-size: 0.6875rem;
		line-height: 1.2;
		white-space: nowrap;
		pointer-events: all;
	}
	/* The label border tracks the single alert level (warning=amber, danger=red),
	   which already folds in both the < 1 h / imminent collapse AND mass-critical —
	   so it never needs to re-derive from data-ttl/data-mass. */
	.edge-label[data-alert='warning'] {
		border-color: var(--alert-warning);
	}
	.edge-label[data-alert='danger'] {
		border-color: var(--alert-danger);
	}
	.wh-type {
		font-weight: 700;
		color: var(--slate-200);
	}
	.mass {
		color: var(--mass-colour);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	/* The TTL glyph. Plain variant: a tinted outline icon. Badge variant: a filled
	   coloured disc with the glyph knocked out — used when the alert fires so it
	   pops regardless of line weight (spec §4). */
	.ttl-glyph {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		color: var(--glyph-colour);
	}
	.ttl-glyph svg {
		width: 100%;
		height: 100%;
		display: block;
	}
	.ttl-glyph.badged {
		width: 16px;
		height: 16px;
	}
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
