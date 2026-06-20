<script lang="ts">
	// NODE LAB variant — "Wanderer-style 2-line + corners". DISPOSABLE wireframe (see
	// NodeLab.svelte). A compact, FLAT take inspired by Wanderer: a left class stripe,
	// a fixed two-line body (class+name / custom name), and metadata clustered in the
	// corners — so the box stays short instead of growing a stack of full-width rows.
	// NO top tab-tag (that's the chain/tab label, not per-node data). Meaning stays in
	// text + shape; colour decorates (Fork 3). Standalone (plain div, no @xyflow Handles)
	// because the lab lays variants out on a static grid, not the live canvas.
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
		<div class="line1">
			<span class="class" style:--c={classColour[system.class]}>{system.class}</span>
			<span class="name">{system.name}</span>
		</div>
		<div class="line2">
			{#if system.custom_name}
				<span class="custom">{system.custom_name}</span>
			{:else}
				<span class="custom muted">—</span>
			{/if}
		</div>
	</div>

	<div class="meta">
		<!-- Top-right: static destination classes, compact. -->
		{#if system.statics.length > 0}
			<div class="statics">
				{#each system.statics as s, i (i)}
					<span class="stat" style:--c={classColour[s.dest]}>{s.dest}</span>
				{/each}
			</div>
		{/if}
		<!-- Bottom-right: status glyph cluster (root + intel flags). -->
		<div class="icons">
			{#if isRoot}
				<span class="ico root-ico" title={m.map_proto_root()} aria-label={m.map_proto_root()}>⚓</span>
			{/if}
			{#each activeFlags as f (f)}
				<span class="ico" style:--c={flagColour[f]} title={flagLabel[f]} aria-label={flagLabel[f]}>
					<span aria-hidden="true">{flagGlyph[f]}</span>
				</span>
			{/each}
		</div>
	</div>
</div>

<style>
	.node {
		position: relative;
		display: flex;
		align-items: stretch;
		min-width: 150px;
		max-width: 220px;
		padding: 0.3rem 0.45rem 0.3rem 0.6rem;
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
	/* Left class stripe — the class colour as a structural bar (not just text colour). */
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
	.line1 {
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
	.line2 {
		font-size: 0.72rem;
	}
	.custom {
		color: var(--sky);
	}
	.custom.muted {
		color: var(--slate-600);
	}

	.meta {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		justify-content: space-between;
		gap: 0.2rem;
		margin-left: 0.4rem;
	}
	.statics {
		display: flex;
		gap: 0.2rem;
	}
	.stat {
		font-size: 0.64rem;
		font-weight: 700;
		color: var(--c);
		border: 1px solid var(--c);
		border-radius: 3px;
		padding: 0 0.22rem;
	}
	.icons {
		display: flex;
		gap: 0.2rem;
		align-items: center;
	}
	.ico {
		font-size: 0.72rem;
		line-height: 1;
		color: var(--c, var(--slate-300));
	}
	.root-ico {
		color: var(--sky);
	}
</style>
