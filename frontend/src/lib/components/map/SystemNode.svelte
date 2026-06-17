<script lang="ts">
	// Custom svelte-flow node — the STYLE seam for a system. Meaning is carried by
	// TEXT (class / security / static destination classes), colour only decorates,
	// so the node stays legible in greyscale, forced-colors, and to a colourblind
	// user (Fork 3).
	import { Handle, Position } from '@xyflow/svelte';
	import { m } from '$lib/paraglide/messages';
	import type { System, SystemClass } from '$lib/map/types';

	// `selected` is supplied by Svelte Flow (NodeProps) — it tracks selection for
	// us, so a "selected system" is just this node in its selected state, not a
	// separate component. The rest comes through `data`.
	let {
		data,
		selected = false
	}: {
		data: { system: System; isRoot: boolean; isGhost: boolean };
		selected?: boolean;
	} = $props();

	const system = $derived(data.system);

	// Class C1–C6 map to --c1..c6; the k-space tiers HS/LS/NS to --hs/--ls/--ns;
	// Pochven (P) to --pochven. The badge always shows the class TEXT; this only
	// picks the decorative colour.
	const classColour: Record<SystemClass, string> = {
		C1: 'var(--c1)',
		C2: 'var(--c2)',
		C3: 'var(--c3)',
		C4: 'var(--c4)',
		C5: 'var(--c5)',
		C6: 'var(--c6)',
		HS: 'var(--hs)',
		LS: 'var(--ls)',
		NS: 'var(--ns)',
		P: 'var(--pochven)'
	};
</script>

<div
	class="system-node"
	class:root={data.isRoot}
	class:ghost={data.isGhost}
	class:selected
	data-class={system.class}
>
	<!-- Floating edges: a `source` handle on every side so getEdgeParams can anchor
	     an edge to whichever side faces the neighbour (see floating-edge.ts), plus a
	     matching `target` handle per side so svelte-flow can resolve both ends of an
	     edge. Handles are hidden — endpoints float to the perimeter, not to a dot. -->
	<Handle type="source" position={Position.Top} id="top" />
	<Handle type="source" position={Position.Left} id="left" />
	<Handle type="target" position={Position.Top} id="t-top" />
	<Handle type="target" position={Position.Left} id="t-left" />

	<header>
		<span class="badge class" style:--badge-colour={classColour[system.class]}>{system.class}</span>
		<span class="name">{system.name}</span>
		{#if data.isRoot}
			<span class="badge root-badge">{m.map_proto_root()}</span>
		{/if}
		{#if data.isGhost}
			<span class="badge ghost-badge">{m.map_proto_ghost()}</span>
		{/if}
	</header>

	{#if system.statics.length > 0}
		<ul class="statics" aria-label="statics">
			<!-- Show the static's DESTINATION class (HS/LS/C5…), not the wormhole-type
			     code — the type isn't user-facing yet (it's kept for the later
			     signature-scanning work). Key by index since a system can have two
			     statics to the same destination. -->
			{#each system.statics as s, i (i)}
				<li class="badge static" style:--badge-colour={classColour[s.dest]}>{s.dest}</li>
			{/each}
		</ul>
	{/if}

	<!-- Selected: the node grows and reveals extra detail in-place. Same data the
	     sidebar's System Intel shows, surfaced on the node so the focus reads at a
	     glance. (Security is a placeholder until the chain-map model supplies it.) -->
	{#if selected}
		<dl class="detail">
			<dt>{m.map_proto_intel_security()}</dt>
			<dd>—</dd>
			<dt>{m.map_proto_intel_statics()}</dt>
			<dd>
				{#if system.statics.length}
					{system.statics.map((s) => s.dest).join(', ')}
				{:else}—{/if}
			</dd>
		</dl>
	{/if}

	<Handle type="source" position={Position.Right} id="right" />
	<Handle type="source" position={Position.Bottom} id="bottom" />
	<Handle type="target" position={Position.Right} id="t-right" />
	<Handle type="target" position={Position.Bottom} id="t-bottom" />
</div>

<style>
	.system-node {
		min-width: 110px;
		padding: var(--space-sm, 0.5rem) 0.6rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		color: var(--slate-100);
		font-family: var(--font-ui);
		font-size: 0.75rem;
		line-height: 1.3;
	}
	/* A root system is anchored — give it a distinct, non-colour-only outline
	   (thicker, accent border) so it reads as the chain anchor even in greyscale. */
	.system-node.root {
		border-color: var(--sky);
		border-width: 2px;
		padding: calc(0.5rem - 1px) calc(0.6rem - 1px);
	}
	/* A ghost (local-only, unconfirmed) reads as provisional: dashed border +
	   reduced opacity. The "unconfirmed" text badge carries the meaning; this is
	   decoration that survives forced-colors via the dashed STYLE, not colour. */
	.system-node.ghost {
		border-style: dashed;
		opacity: 0.8;
	}
	/* The selected system — a violet highlight ring drawn with box-shadow so it
	   composes with the root border (a node can be both root AND selected). The
	   ring also survives forced-colors (it's an outline-like shadow, not a token
	   fill); selection is additionally reflected in the sidebar's System Intel. */
	.system-node.selected {
		box-shadow: 0 0 0 2px var(--violet);
		border-color: var(--violet);
		/* Grow the focused node so the revealed detail has room and the selection
		   reads at a glance. */
		min-width: 150px;
		z-index: 1;
	}

	/* Extra detail revealed only on the selected node. */
	.detail {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 2px 10px;
		margin: 0.4rem 0 0;
		padding-top: 0.4rem;
		border-top: 1px solid var(--space-700);
		font-size: 0.6875rem;
	}
	.detail dt {
		color: var(--slate-500);
	}
	.detail dd {
		margin: 0;
		color: var(--slate-300);
	}

	header {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		flex-wrap: wrap;
	}
	.name {
		font-weight: 600;
	}

	.badge {
		display: inline-block;
		padding: 0 0.3rem;
		border-radius: 3px;
		font-size: 0.6875rem;
		font-weight: 700;
		white-space: nowrap;
	}
	/* Class/static badges: the colour DECORATES the border + text; the class text
	   (system class, or static destination class) is the real signal. */
	.badge.class,
	.badge.static {
		color: var(--badge-colour);
		border: 1px solid var(--badge-colour);
	}
	.badge.root-badge {
		color: var(--sky);
		border: 1px solid var(--sky);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.badge.ghost-badge {
		color: var(--slate-400);
		border: 1px dashed var(--slate-400);
	}

	.statics {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
		margin: 0.35rem 0 0;
		padding: 0;
		list-style: none;
	}

	/* Class colours are decoration; under forced-colors the OS flattens them, but
	   the code TEXT and the structural border survive, so nothing is lost. We do
	   NOT opt out with forced-color-adjust here — text already carries meaning. */

	/* Floating edges anchor to the node's perimeter, not to a visible dot — hide
	   the handles (they exist only so getEdgeParams can resolve a side). */
	.system-node :global(.svelte-flow__handle) {
		opacity: 0;
		min-width: 1px;
		min-height: 1px;
		width: 1px;
		height: 1px;
		border: 0;
		pointer-events: none;
	}
</style>
