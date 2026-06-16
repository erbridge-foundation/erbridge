<script lang="ts">
	// Test harness: mounts SystemNode / ConnectionEdge inside a real SvelteFlow so
	// the <Handle> / <EdgeLabelRenderer> node-context requirement is satisfied. The
	// component tests render THIS and assert on the encoding the custom components
	// produce (class/sec/static text, mass text cue, EoL glyph) — see
	// SystemNode.test.ts / ConnectionEdge.test.ts.
	import { SvelteFlow } from '@xyflow/svelte';
	import type { Node, Edge, NodeTypes, EdgeTypes } from '@xyflow/svelte';
	import SystemNode from './SystemNode.svelte';
	import ConnectionEdge from './ConnectionEdge.svelte';

	let { nodes = [], edges = [] }: { nodes?: Node[]; edges?: Edge[] } = $props();

	const nodeTypes: NodeTypes = { system: SystemNode };
	const edgeTypes: EdgeTypes = { connection: ConnectionEdge };

	// Local mutable copies SvelteFlow can bind into; seeded once from the props
	// (a test harness — props don't change after mount).
	// svelte-ignore state_referenced_locally
	let n = $state<Node[]>([...nodes]);
	// svelte-ignore state_referenced_locally
	let e = $state<Edge[]>([...edges]);
</script>

<div style="width: 800px; height: 600px;">
	<SvelteFlow bind:nodes={n} bind:edges={e} {nodeTypes} {edgeTypes} />
</div>
