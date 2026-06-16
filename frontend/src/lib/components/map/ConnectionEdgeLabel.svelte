<script lang="ts">
	// The encoding-bearing label of a connection edge, factored OUT of
	// ConnectionEdge.svelte so it can be unit-tested without SvelteFlow's edge
	// pipeline (which needs measured node dimensions jsdom can't provide). This
	// component owns the Fork-3 rules: wormhole type, a mass TEXT cue, and an EoL
	// ⚠ glyph + screen-reader text — none of which depend on colour or motion.
	import { m } from '$lib/paraglide/messages';
	import type { Mass } from '$lib/map/types';

	let {
		wh_type,
		mass,
		eol,
		showMass = true,
		showWhType = true
	}: {
		wh_type: string;
		mass: Mass;
		eol: boolean;
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
</script>

<div class="edge-label" class:eol data-mass={mass}>
	{#if showWhType}
		<span class="wh-type">{wh_type}</span>
	{/if}
	{#if showMass}
		<!-- Mass as TEXT — survives forced-colors and colourblindness. -->
		<span class="mass" style:--mass-colour={massColour[mass]}>{massText[mass]}</span>
	{/if}
	{#if eol}
		<!-- ⚠ glyph carries EoL without colour OR motion; the pulse is decoration. -->
		<span class="eol-flag" title={m.map_proto_eol()}>
			<span class="pulse" aria-hidden="true"></span>
			<span aria-hidden="true">⚠</span>
			<span class="sr-only">{m.map_proto_eol()}</span>
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
	.edge-label.eol {
		border-color: var(--mass-critical);
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
	.eol-flag {
		position: relative;
		display: inline-flex;
		align-items: center;
		color: var(--mass-critical);
		font-weight: 700;
	}
	/* Pulse is decoration only. It drops under reduced-motion (the global app.css
	   rule kills the animation duration) with NO information loss — the ⚠ glyph
	   and the screen-reader text both still convey end-of-life. */
	.pulse {
		position: absolute;
		inset: -3px;
		border-radius: 50%;
		border: 1px solid var(--mass-critical);
		animation: eol-pulse 1.4s ease-out infinite;
	}
	@keyframes eol-pulse {
		0% {
			transform: scale(0.6);
			opacity: 0.8;
		}
		100% {
			transform: scale(1.4);
			opacity: 0;
		}
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
