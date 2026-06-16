<script lang="ts">
	// Custom svelte-flow edge — the STYLE seam for a connection. FLOATING edge: the
	// endpoints aren't pinned to fixed handles, they float to the point on each
	// node's perimeter that faces the other node (getEdgeParams), then a bezier
	// curve is drawn between them. The stroke is decorated by mass / EoL; the
	// label's CONTENT + colour-independent encoding live in ConnectionEdgeLabel.
	import {
		BaseEdge,
		EdgeLabel,
		getBezierPath,
		useInternalNode,
		type EdgeProps
	} from '@xyflow/svelte';
	import ConnectionEdgeLabel from './ConnectionEdgeLabel.svelte';
	import { getEdgeParams } from './floating-edge';
	import type { Mass } from '$lib/map/types';

	type ConnData = {
		wh_type: string;
		mass: Mass;
		eol: boolean;
		thickness?: number;
		showMass?: boolean;
		showWhType?: boolean;
	};

	let { source, target, markerEnd, data }: EdgeProps = $props();

	// svelte-flow types `data` as optional `any`; narrow + default it.
	const d = $derived((data ?? { wh_type: '', mass: 'fresh', eol: false }) as ConnData);

	// source/target are stable for an edge's lifetime; pass them once to the hook.
	// svelte-ignore state_referenced_locally
	const sourceNode = useInternalNode(source);
	// svelte-ignore state_referenced_locally
	const targetNode = useInternalNode(target);

	// Floating bezier path + its midpoint (for the label). Recomputed reactively
	// as either node is dragged, so the connection point migrates around the node.
	const geom = $derived.by(() => {
		if (!sourceNode.current || !targetNode.current) return null;
		const p = getEdgeParams(sourceNode.current, targetNode.current);
		const [path, labelX, labelY] = getBezierPath({
			sourceX: p.sx,
			sourceY: p.sy,
			sourcePosition: p.sourcePos,
			targetX: p.tx,
			targetY: p.ty,
			targetPosition: p.targetPos
		});
		return { path, labelX, labelY };
	});

	const massColour: Record<Mass, string> = {
		fresh: 'var(--mass-fresh)',
		half: 'var(--mass-half)',
		critical: 'var(--mass-critical)'
	};
	const stroke = $derived(d.eol ? 'var(--mass-critical)' : massColour[d.mass]);
	const thickness = $derived(d.thickness ?? 2);
	// The label shows nothing when both cues are toggled off — skip it entirely.
	const showLabel = $derived((d.showMass ?? true) || (d.showWhType ?? true) || d.eol);
</script>

{#if geom}
	<BaseEdge
		{markerEnd}
		path={geom.path}
		style="stroke: {stroke}; stroke-width: {thickness}; {d.eol ? 'stroke-dasharray: 6 4;' : ''}"
	/>

	{#if showLabel}
		<EdgeLabel x={geom.labelX} y={geom.labelY} transparent>
			<ConnectionEdgeLabel
				wh_type={d.wh_type}
				mass={d.mass}
				eol={d.eol}
				showMass={d.showMass ?? true}
				showWhType={d.showWhType ?? true}
			/>
		</EdgeLabel>
	{/if}
{/if}
