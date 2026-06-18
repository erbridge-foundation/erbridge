<script lang="ts">
	// The encoding-bearing label of a connection edge, factored OUT of
	// ConnectionEdge.svelte so it can be unit-tested without SvelteFlow's edge
	// pipeline (which needs measured node dimensions jsdom can't provide). It owns
	// the COLOUR-INDEPENDENT half of the encoding: the wormhole type, a mass TEXT
	// cue, and the precise TTL state as sr-only TEXT — none of which rely on hue or
	// motion (greyscale + reduced-motion both stay legible — spec §1/§6/§7). The TTL
	// MID-EDGE glyph was dropped (the dashed line + breathing casing carry TTL, and
	// the midpoint is now the direction arrow's home); the sr-only text keeps the
	// precise four-state TTL in the DOM so the state survives loss of colour/motion.
	import { m } from '$lib/paraglide/messages';
	import type { Mass, TtlState } from '$lib/map/types';

	let {
		wh_type,
		mass,
		ttlBucket,
		alertLevel,
		showMass = true,
		showWhType = true,
		title
	}: {
		wh_type: string;
		mass: Mass;
		ttlBucket: TtlState;
		alertLevel: 'none' | 'warning' | 'danger';
		showMass?: boolean;
		showWhType?: boolean;
		/** The connection's hover tooltip, inherited from the edge so a visible pill
		 *  surfaces the same status as the line. */
		title?: string;
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

	// Human-readable TTL label, surfaced as sr-only text on non-stable edges so the
	// precise four-state time (lt1h vs imminent) survives loss of colour AND motion —
	// the glyph that used to carry it is gone.
	const ttlText: Record<TtlState, string> = {
		stable: m.map_proto_ttl_stable(),
		lt4h: m.map_proto_ttl_lt4h(),
		lt1h: m.map_proto_ttl_lt1h(),
		imminent: m.map_proto_ttl_imminent()
	};

	// Whether any VISIBLE cue renders — the wh-type only counts when non-empty (the
	// named-type derivation yields '' for an all-K162/unscanned hole). When nothing is
	// visible the chrome (border/background/padding) is dropped so a label carrying
	// only the sr-only TTL text doesn't paint an empty residual pill on the edge.
	const hasVisible = $derived((showWhType && wh_type !== '') || showMass);
</script>

<div
	class="edge-label"
	class:chrome={hasVisible}
	data-alert={alertLevel}
	data-mass={mass}
	data-ttl={ttlBucket}
	{title}
>
	{#if showWhType && wh_type !== ''}
		<span class="wh-type">{wh_type}</span>
	{/if}
	{#if showMass}
		<!-- Mass as TEXT — survives forced-colors and colourblindness. -->
		<span class="mass" style:--mass-colour={massColour[mass]}>{massText[mass]}</span>
	{/if}
	{#if ttlBucket !== 'stable'}
		<!-- The precise four-state TTL as sr-only text (the mid-edge glyph that used
		     to carry it is gone). Rendered for every non-stable edge regardless of the
		     mass/wh-type toggles, so the time state survives loss of colour + motion
		     even on a label stripped of its visible cues. -->
		<span class="sr-only">{ttlText[ttlBucket]}</span>
	{/if}
</div>

<style>
	.edge-label {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		font-family: var(--font-ui);
		font-size: 0.6875rem;
		line-height: 1.2;
		white-space: nowrap;
		pointer-events: all;
	}
	/* The pill chrome (background/border/padding) is applied ONLY when a visible cue
	   renders — a label carrying just the sr-only TTL text stays a bare, invisible
	   wrapper instead of painting an empty residual pill on the edge. */
	.edge-label.chrome {
		padding: 0.1rem 0.35rem;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
	}
	/* The label border tracks the single alert level (warning=amber, danger=red),
	   which already folds in both the < 1 h / imminent collapse AND mass-critical —
	   so it never needs to re-derive from data-ttl/data-mass. */
	.edge-label.chrome[data-alert='warning'] {
		border-color: var(--alert-warning);
	}
	.edge-label.chrome[data-alert='danger'] {
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
