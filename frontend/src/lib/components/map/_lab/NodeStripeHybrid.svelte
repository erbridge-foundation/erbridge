<script lang="ts">
	// NODE LAB variant — "Left-stripe hybrid". DISPOSABLE wireframe (see NodeLab.svelte).
	// Keeps a small STACKED body (class+name on top, custom name under) like today's node,
	// but trims the fat: a left class stripe carries the class colour structurally, and
	// flags/statics move into a tight RIGHT-corner cluster instead of full-width wrapping
	// rows. Standalone (plain div, no @xyflow Handles).
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

<div
	class="node"
	class:root={isRoot}
	class:selected
	style:--stripe={classColour[system.class]}
	data-class={system.class}
>
	<span class="stripe" aria-hidden="true"></span>

	<div class="body">
		<div class="head">
			<span class="class" style:--c={classColour[system.class]}>{system.class}</span>
			<span class="name">{system.name}</span>
			{#if isRoot}
				<span class="root-badge">{m.map_proto_root()}</span>
			{/if}
		</div>
		{#if system.custom_name}
			<div class="custom">{system.custom_name}</div>
		{/if}
	</div>

	<div class="cluster">
		{#each system.statics as s, i (i)}
			<span class="stat" style:--c={classColour[s.dest]}>{s.dest}</span>
		{/each}
		{#each activeFlags as f (f)}
			<span class="ico" style:--c={flagColour[f]} title={flagLabel[f]} aria-label={flagLabel[f]}>
				<span aria-hidden="true">{flagGlyph[f]}</span>
			</span>
		{/each}
	</div>
</div>

<style>
	.node {
		position: relative;
		display: flex;
		align-items: center;
		gap: 0.4rem;
		min-width: 150px;
		max-width: 230px;
		padding: 0.32rem 0.45rem 0.32rem 0.6rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		color: var(--slate-100);
		font-family: var(--font-ui);
		font-size: 0.78rem;
		line-height: 1.25;
		overflow: hidden;
	}
	.node.root {
		border-color: var(--sky);
	}
	.node.selected {
		box-shadow: 0 0 0 2px var(--violet);
		border-color: var(--violet);
	}
	.stripe {
		position: absolute;
		inset: 0 auto 0 0;
		width: 4px;
		background: var(--stripe);
		border-radius: 6px 0 0 6px;
	}

	.body {
		flex: 1;
		min-width: 0;
		padding-left: 0.25rem;
	}
	.head {
		display: flex;
		align-items: baseline;
		gap: 0.3rem;
		white-space: nowrap;
	}
	.class {
		font-weight: 700;
		color: var(--c);
	}
	.name {
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.root-badge {
		font-size: 0.6rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--sky);
		border: 1px solid var(--sky);
		border-radius: 3px;
		padding: 0 0.2rem;
	}
	.custom {
		font-size: 0.72rem;
		color: var(--sky);
	}

	.cluster {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 0.22rem;
		max-width: 64px;
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
		font-size: 0.74rem;
		line-height: 1;
		color: var(--c);
	}
</style>
