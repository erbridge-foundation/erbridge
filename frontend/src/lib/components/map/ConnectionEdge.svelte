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
		/** Sig ids at the a (source) / b (target) ends; absent when unscanned. */
		sig_a?: string;
		sig_b?: string;
		/** Which end the direction arrow points TO ('a'=source, 'b'=target), or
		 *  null when the direction is undetermined (both ends unidentified). */
		arrowTo?: 'a' | 'b' | null;
		showDirection?: boolean;
		thickness?: number;
		showMass?: boolean;
		showWhType?: boolean;
		/** This edge's slot within its parallel group (0-based) and the group size.
		 *  When >1, sibling edges between the same node pair bow apart so they don't
		 *  stack — a "bidirectional"-style lens, no arrows. */
		parallelIndex?: number;
		parallelCount?: number;
	};

	// Perpendicular separation (px) between adjacent parallel siblings.
	const PARALLEL_GAP = 26;

	let { source, target, markerEnd, markerStart, data }: EdgeProps = $props();

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

		// Bow offset for parallel siblings: spread slots symmetrically around the
		// centre line (e.g. 2 edges → ±half-gap; 3 → -gap, 0, +gap) along the
		// PERPENDICULAR of the source→target line.
		const count = d.parallelCount ?? 1;
		const index = d.parallelIndex ?? 0;
		let bowX = 0;
		let bowY = 0;
		if (count > 1) {
			const slot = index - (count - 1) / 2; // centred slot, e.g. -0.5, +0.5
			const dx = p.tx - p.sx;
			const dy = p.ty - p.sy;
			const len = Math.hypot(dx, dy) || 1;
			const offset = slot * PARALLEL_GAP;
			bowX = (-dy / len) * offset; // unit perpendicular × offset
			bowY = (dx / len) * offset;
		}

		let path: string;
		let labelX: number;
		let labelY: number;
		if (count > 1) {
			// Quadratic curve bowing through the offset midpoint, so the two (or
			// more) holes between a pair separate into a lens — no arrows. The
			// control point is offset by 2× the bow so the curve's apex lands at it.
			const ctrlX = (p.sx + p.tx) / 2 + bowX * 2;
			const ctrlY = (p.sy + p.ty) / 2 + bowY * 2;
			path = `M ${p.sx},${p.sy} Q ${ctrlX},${ctrlY} ${p.tx},${p.ty}`;
			// A quadratic's apex sits halfway between the chord midpoint and the
			// control point — put the label there so it rides the curve.
			labelX = (p.sx + p.tx) / 4 + ctrlX / 2;
			labelY = (p.sy + p.ty) / 4 + ctrlY / 2;
		} else {
			[path, labelX, labelY] = getBezierPath({
				sourceX: p.sx,
				sourceY: p.sy,
				sourcePosition: p.sourcePos,
				targetX: p.tx,
				targetY: p.ty,
				targetPosition: p.targetPos
			});
		}

		// Endpoint label anchors: a short way in from each node along the straight
		// source→target line, nudged toward this edge's bow so each sibling's sig
		// pills track its own curve.
		const sigSourceX = p.sx + (p.tx - p.sx) * SIG_INSET + bowX;
		const sigSourceY = p.sy + (p.ty - p.sy) * SIG_INSET + bowY;
		const sigTargetX = p.tx + (p.sx - p.tx) * SIG_INSET + bowX;
		const sigTargetY = p.ty + (p.sy - p.ty) * SIG_INSET + bowY;
		return {
			path,
			labelX,
			labelY,
			sigSourceX,
			sigSourceY,
			sigTargetX,
			sigTargetY,
			sx: p.sx,
			sy: p.sy,
			tx: p.tx,
			ty: p.ty,
			midX: (p.sx + p.tx) / 2 + bowX,
			midY: (p.sy + p.ty) / 2 + bowY
		};
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

	// Direction marker. The arrowhead toward the K162 end is the built-in
	// markerEnd/markerStart (set in MapCanvas, applied to BaseEdge below) — it's
	// tangent-accurate and hugs the node. When "show direction" is on but the
	// direction is UNDETERMINED (arrowTo null), draw a neutral mid-edge diamond.
	const neutralMarker = $derived(
		geom && d.showDirection && d.arrowTo == null ? { x: geom.midX, y: geom.midY } : null
	);
</script>

{#if geom}
	<!-- The direction arrowhead (when known) is the built-in markerEnd/markerStart,
	     applied here so it auto-orients to the path tangent and hugs the node. -->
	<BaseEdge
		path={geom.path}
		{markerEnd}
		{markerStart}
		style="stroke: {stroke}; stroke-width: {thickness}; {d.eol ? 'stroke-dasharray: 6 4;' : ''}"
	/>

	<!-- Undetermined direction: a neutral mid-edge diamond instead of an arrow. -->
	{#if neutralMarker}
		<rect
			class="dir-neutral"
			x={neutralMarker.x - 4}
			y={neutralMarker.y - 4}
			width="8"
			height="8"
			transform="rotate(45 {neutralMarker.x} {neutralMarker.y})"
			fill={stroke}
		/>
	{/if}

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
	{#if d.sig_a}
		<EdgeLabel x={geom.sigSourceX} y={geom.sigSourceY} transparent>
			<span class="sig-endpoint">{sigLabel(d.sig_a)}</span>
		</EdgeLabel>
	{/if}
	{#if d.sig_b}
		<EdgeLabel x={geom.sigTargetX} y={geom.sigTargetY} transparent>
			<span class="sig-endpoint">{sigLabel(d.sig_b)}</span>
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
