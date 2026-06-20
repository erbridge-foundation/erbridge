<script lang="ts">
	// NODE LAB variant — "Minimal / icon". DISPOSABLE wireframe (see NodeLab.svelte).
	// The slimmest take: a class-coloured dot + the name, and nothing else inline. Flags,
	// statics and the custom name all move to hover (title) / the sidebar — so the node is
	// as small as it can be while still being identifiable on a dense map. A faint flag dot
	// hints "this system has markers" without spelling them out. Standalone (plain div).
	import { m } from '$lib/paraglide/messages';
	import { INTEL_FLAGS, type System } from '$lib/map/types';
	import { classColour, flagLabel } from './node-lab-tokens';

	let {
		system,
		isRoot = false,
		selected = false
	}: { system: System; isRoot?: boolean; selected?: boolean } = $props();

	const activeFlags = $derived(INTEL_FLAGS.filter((f) => system.flags?.includes(f) ?? false));
	// Tooltip rolls up the detail this variant hides inline.
	const tip = $derived(
		[
			`${system.class} ${system.name}`,
			system.custom_name,
			system.statics.length ? `↗ ${system.statics.map((s) => s.dest).join(', ')}` : '',
			activeFlags.map((f) => flagLabel[f]).join(', ')
		]
			.filter(Boolean)
			.join(' · ')
	);
</script>

<div
	class="node"
	class:root={isRoot}
	class:selected
	style:--dot={classColour[system.class]}
	data-class={system.class}
	title={tip}
>
	<span class="dot" aria-hidden="true"></span>
	<span class="class">{system.class}</span>
	<span class="name">{system.name}</span>
	{#if isRoot}
		<span class="root-ico" title={m.map_proto_root()} aria-label={m.map_proto_root()}>⚓</span>
	{/if}
	{#if activeFlags.length > 0}
		<span class="flagdot" aria-hidden="true" title={activeFlags.map((f) => flagLabel[f]).join(', ')}
		></span>
	{/if}
</div>

<style>
	.node {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.22rem 0.5rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 999px;
		color: var(--slate-100);
		font-family: var(--font-ui);
		font-size: 0.78rem;
		line-height: 1.1;
		white-space: nowrap;
	}
	.node.root {
		border-color: var(--sky);
	}
	.node.selected {
		box-shadow: 0 0 0 2px var(--violet);
		border-color: var(--violet);
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--dot);
		flex: none;
	}
	.class {
		font-weight: 700;
		color: var(--dot);
	}
	.name {
		font-weight: 600;
	}
	.root-ico {
		color: var(--sky);
		font-size: 0.72rem;
	}
	.flagdot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--amber);
		flex: none;
	}
</style>
