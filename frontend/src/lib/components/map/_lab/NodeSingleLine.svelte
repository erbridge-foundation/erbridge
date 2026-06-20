<script lang="ts">
	// NODE LAB variant — "Single-line dense". DISPOSABLE wireframe (see NodeLab.svelte).
	// The shortest take: ONE row — class chip + name + inline mini-glyphs for statics +
	// intel flags. The custom name (when present) trails faint after the name; full detail
	// lives in the sidebar/tooltip. Standalone (plain div, no @xyflow Handles).
	import { m } from '$lib/paraglide/messages';
	import { INTEL_FLAGS, type System } from '$lib/map/types';
	import { classColour, flagGlyph, flagColour, flagLabel } from './node-lab-tokens';

	let {
		system,
		isRoot = false,
		selected = false
	}: { system: System; isRoot?: boolean; selected?: boolean } = $props();

	const activeFlags = $derived(INTEL_FLAGS.filter((f) => system.flags?.includes(f) ?? false));
</script>

<div class="node" class:root={isRoot} class:selected data-class={system.class}>
	<span class="class" style:--c={classColour[system.class]}>{system.class}</span>
	<span class="name">{system.name}</span>
	{#if system.custom_name}
		<span class="custom" title={system.custom_name}>· {system.custom_name}</span>
	{/if}

	<span class="spacer"></span>

	{#if isRoot}
		<span class="ico root-ico" title={m.map_proto_root()} aria-label={m.map_proto_root()}>⚓</span>
	{/if}
	{#each system.statics as s, i (i)}
		<span class="stat" style:--c={classColour[s.dest]}>{s.dest}</span>
	{/each}
	{#each activeFlags as f (f)}
		<span class="ico" style:--c={flagColour[f]} title={flagLabel[f]} aria-label={flagLabel[f]}>
			<span aria-hidden="true">{flagGlyph[f]}</span>
		</span>
	{/each}
</div>

<style>
	.node {
		display: flex;
		align-items: center;
		gap: 0.3rem;
		min-width: 150px;
		max-width: 260px;
		padding: 0.28rem 0.5rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		color: var(--slate-100);
		font-family: var(--font-ui);
		font-size: 0.78rem;
		line-height: 1.2;
		white-space: nowrap;
	}
	.node.root {
		border-color: var(--sky);
	}
	.node.selected {
		box-shadow: 0 0 0 2px var(--violet);
		border-color: var(--violet);
	}
	.class {
		font-weight: 700;
		color: var(--c);
	}
	.name {
		font-weight: 600;
	}
	.custom {
		color: var(--sky);
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
	}
	.spacer {
		flex: 1;
	}
	.stat {
		font-size: 0.64rem;
		font-weight: 700;
		color: var(--c);
		border: 1px solid var(--c);
		border-radius: 3px;
		padding: 0 0.22rem;
	}
	.ico {
		font-size: 0.78rem;
		line-height: 1;
		color: var(--c, var(--slate-300));
	}
	.root-ico {
		color: var(--sky);
	}
</style>
