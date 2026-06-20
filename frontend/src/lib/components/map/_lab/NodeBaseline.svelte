<script lang="ts">
	// NODE LAB variant 1 — the BASELINE: today's real SystemNode, shown for comparison.
	// SystemNode uses <Handle>, which needs Svelte-Flow context, so we mount it inside a
	// tiny non-interactive <SvelteFlow> (mirrors the component test harness). DISPOSABLE.
	import { SvelteFlow, type Node, type NodeTypes } from '@xyflow/svelte';
	import SystemNode from '../SystemNode.svelte';
	import type { System } from '$lib/map/types';

	let {
		system,
		isRoot = false,
		selected = false
	}: { system: System; isRoot?: boolean; selected?: boolean } = $props();

	const nodeTypes: NodeTypes = { system: SystemNode };
	// svelte-ignore state_referenced_locally
	let nodes = $state<Node[]>([
		{
			id: system.id,
			type: 'system',
			position: { x: 8, y: 8 },
			selected,
			data: { system, isRoot, isGhost: false }
		}
	]);
	let edges = $state([]);
</script>

<!-- A fixed little stage just big enough for the node; interaction disabled so it reads
     as a static card, not a draggable canvas. -->
<div class="stage">
	<SvelteFlow
		bind:nodes
		bind:edges
		{nodeTypes}
		nodesDraggable={false}
		nodesConnectable={false}
		elementsSelectable={false}
		panOnDrag={false}
		zoomOnScroll={false}
		zoomOnPinch={false}
		zoomOnDoubleClick={false}
		preventScrolling={false}
		fitView={false}
	/>
</div>

<style>
	.stage {
		width: 100%;
		height: 80px;
		pointer-events: none;
	}
	/* Hide the canvas chrome (attribution, the grid) so only the node shows. */
	.stage :global(.svelte-flow__attribution),
	.stage :global(.svelte-flow__background) {
		display: none;
	}
	.stage :global(.svelte-flow) {
		background: transparent;
	}
</style>
