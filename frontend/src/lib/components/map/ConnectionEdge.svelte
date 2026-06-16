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
		sig_source?: string;
		sig_target?: string;
		thickness?: number;
		showMass?: boolean;
		showWhType?: boolean;
	};

	let { source, target, markerEnd, data }: EdgeProps = $props();

	// svelte-flow types `data` as optional `any`; narrow + default it.
	const d = $derived((data ?? { wh_type: '', mass: 'fresh', eol: false }) as ConnData);

	// How the sig id is shown on the endpoint pill. For now: the first 3 chars
	// (`ABC-123` → `ABC`). This will become a per-map preference later.
	const sigLabel = (id: string) => id.slice(0, 3);

	// source/target are stable for an edge's lifetime; pass them once to the hook.
	// svelte-ignore state_referenced_locally
	const sourceNode = useInternalNode(source);
	// svelte-ignore state_referenced_locally
	const targetNode = useInternalNode(target);

	// Floating bezier path + its midpoint (for the label). Recomputed reactively
	// as either node is dragged, so the connection point migrates around the node.
	// How far (px) from a node's perimeter the sig endpoint label sits, nudged
	// along the edge toward the midpoint so it hugs the node like the wireframe.
	const SIG_INSET = 0.16;

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
		// Endpoint label anchors: a short way in from each node along the straight
		// source→target line (close enough to read as "this sig, this side").
		const sigSourceX = p.sx + (p.tx - p.sx) * SIG_INSET;
		const sigSourceY = p.sy + (p.ty - p.sy) * SIG_INSET;
		const sigTargetX = p.tx + (p.sx - p.tx) * SIG_INSET;
		const sigTargetY = p.ty + (p.sy - p.ty) * SIG_INSET;
		return { path, labelX, labelY, sigSourceX, sigSourceY, sigTargetX, sigTargetY };
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

	<!-- Sig endpoint labels: which signature in each system leads to this hole.
	     Svelte Flow supports multiple <EdgeLabel>s per edge — these two sit near
	     the endpoints, the type/mass label sits at the midpoint above. -->
	{#if d.sig_source}
		<EdgeLabel x={geom.sigSourceX} y={geom.sigSourceY} transparent>
			<span class="sig-endpoint">{sigLabel(d.sig_source)}</span>
		</EdgeLabel>
	{/if}
	{#if d.sig_target}
		<EdgeLabel x={geom.sigTargetX} y={geom.sigTargetY} transparent>
			<span class="sig-endpoint">{sigLabel(d.sig_target)}</span>
		</EdgeLabel>
	{/if}
{/if}

<style>
	.sig-endpoint {
		display: inline-block;
		padding: 1px 4px;
		background: var(--space-900);
		border: 1px solid var(--space-600);
		border-radius: 3px;
		font-family: var(--font-ui);
		font-size: 9px;
		font-weight: 700;
		line-height: 1.3;
		color: var(--slate-300);
		white-space: nowrap;
		pointer-events: none;
	}
</style>
