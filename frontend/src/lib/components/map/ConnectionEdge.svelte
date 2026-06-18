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
	import { m } from '$lib/paraglide/messages';
	import type { Mass, TtlState } from '$lib/map/types';
	import { resolveEdgeEncoding } from '$lib/map/edge-encoding';

	type ConnData = {
		wh_type: string;
		mass: Mass;
		eol: boolean;
		/** Minutes of life left; buckets into the TTL state that drives dash/glyph/
		 *  alert (see edge-encoding.ts). */
		ttl_remaining_min: number;
		/** Sig ids at the a (source) / b (target) ends; absent when unscanned. */
		sig_a?: string;
		sig_b?: string;
		/** Which end the direction arrow points TO ('a'=source, 'b'=target), or
		 *  null when the direction is undetermined (both ends unidentified). */
		arrowTo?: 'a' | 'b' | null;
		showDirection?: boolean;
		/** User-tunable BASE thickness; the mass encoding overrides per-state width,
		 *  but this still scales the floor so the corp slider keeps an effect. */
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
	const PARALLEL_GAP = 44;

	let { source, target, data }: EdgeProps = $props();

	// svelte-flow types `data` as optional `any`; narrow + default it.
	const d = $derived(
		(data ?? { wh_type: '', mass: 'fresh', eol: false, ttl_remaining_min: 1440 }) as ConnData
	);

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
	// The direction arrow lives at the midpoint (labelX/labelY). When the centre
	// text label is also present, nudge the TEXT this far (px) perpendicular off the
	// line so the rotated arrow keeps the exact midpoint to itself; the arrow alone
	// (label hidden) sits dead-centre, unnudged.
	const LABEL_NUDGE = 14;

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
		// Unit perpendicular of the source→target line — used to push the centre TEXT
		// label off-line so the rotated arrow can own the exact midpoint.
		const dx = p.tx - p.sx;
		const dy = p.ty - p.sy;
		const len = Math.hypot(dx, dy) || 1;
		const perpX = -dy / len;
		const perpY = dx / len;
		return {
			path,
			labelX,
			labelY,
			perpX,
			perpY,
			sigSourceX,
			sigSourceY,
			sigTargetX,
			sigTargetY,
			sx: p.sx,
			sy: p.sy,
			tx: p.tx,
			ty: p.ty
		};
	});

	// The ONE resolver (edge-encoding.ts) turns the two raw variables (mass +
	// remaining-minutes) into every channel: line width/colour, dash, glyph, and
	// the derived alert casing + breathing. Palette swap is done in CSS (a wrapper
	// attribute swaps the three mass hues), so it isn't threaded through here.
	const enc = $derived(resolveEdgeEncoding(d.mass, d.ttl_remaining_min));

	const stroke = $derived(enc.mass.colourVar);
	// Mass owns the width, but the corp thickness slider still scales it: treat the
	// slider as a floor multiplier so people who want fatter/thinner lines keep the
	// control, without losing the relative fresh>half>critical ordering.
	const thickness = $derived(enc.mass.width * ((d.thickness ?? 2) / 2));
	// The label shows nothing when both cues are toggled off AND there's no alert
	// worth surfacing — skip it entirely otherwise.
	const showLabel = $derived(
		(d.showMass ?? true) || (d.showWhType ?? true) || enc.alert.level !== 'none'
	);
	// Whether the centre label paints a VISIBLE pill (mirrors ConnectionEdgeLabel's
	// chrome rule: a non-empty wh-type or the mass cue). When false the label is still
	// mounted on an alert edge — but only to carry its sr-only TTL text, so it must NOT
	// intercept the line hover at the midpoint (the invisible div was stealing the
	// pointer there, hand cursor + no tooltip). Drives both the off-line nudge and the
	// wrapper's pointer-events below.
	const labelVisible = $derived(((d.showWhType ?? true) && d.wh_type !== '') || (d.showMass ?? true));

	// Native hover tooltip for the connection's status (type · mass · TTL state). It
	// USED to hang off the mid-edge TTL glyph; that glyph is gone, so the tooltip now
	// rides the LINE itself — a wide transparent hit-path below carries an SVG <title>
	// so hovering anywhere along the stroke surfaces the status (a richer companion to
	// the always-present sr-only text on the label).
	const massText: Record<Mass, string> = {
		fresh: m.map_proto_mass_fresh(),
		half: m.map_proto_mass_half(),
		critical: m.map_proto_mass_critical()
	};
	const ttlText: Record<TtlState, string> = {
		stable: m.map_proto_ttl_stable(),
		lt4h: m.map_proto_ttl_lt4h(),
		lt1h: m.map_proto_ttl_lt1h(),
		imminent: m.map_proto_ttl_imminent()
	};
	const connTitle = $derived(
		[d.wh_type, m.map_proto_conn_tooltip_mass({ mass: massText[d.mass] }), ttlText[enc.ttlBucket]]
			.filter(Boolean)
			.join(' · ')
	);

	// Direction is shown by a single ➤ arrow at the MIDPOINT (where the dropped TTL
	// glyph used to sit), rotated to lie along the line pointing from the named
	// ("from"/non-K162) end toward the K162 end. No endpoint arrowhead. `arrowTo` is
	// the K162 end → the named end is the other one; undetermined (arrowTo null) → no
	// arrow (a meaningful absence: direction unknown). The arrow is the future hook
	// for a tooltip surfacing the named/K162 sig data.
	const namedEnd = $derived<'a' | 'b' | null>(
		!d.showDirection || d.arrowTo == null ? null : d.arrowTo === 'a' ? 'b' : 'a'
	);
	// Screen-space angle (deg) from the named end toward the K162 end, so the glyph
	// rotates to lie along the line. `a` points sx→tx; `b` points tx→sx.
	const dirAngle = $derived.by(() => {
		if (!geom || namedEnd == null) return 0;
		const [fx, fy, tx2, ty2] =
			namedEnd === 'a'
				? [geom.sx, geom.sy, geom.tx, geom.ty]
				: [geom.tx, geom.ty, geom.sx, geom.sy];
		return (Math.atan2(ty2 - fy, tx2 - fx) * 180) / Math.PI;
	});
</script>

{#if geom}
	<!-- ALERT CASING: a wider, translucent under-stroke drawn BELOW the main line
	     (not a blur filter), owning "attention" for the TTL alert (PURE TTL —
	     mass adds no glow). Only the
	     halo breathes (a CSS class keyed off ttl); the line/dash/label stay still.
	     Under prefers-reduced-motion the global app.css rule kills the animation,
	     and the resting width/opacity are set to the breath MIDPOINT so the static
	     halo doesn't read dimmer than the animated one (spec §6). -->
	{#if enc.alert.level !== 'none'}
		<BaseEdge
			path={geom.path}
			interactionWidth={0}
			class="edge-casing {enc.alert.breatheClass}"
			style="stroke: {enc.alert.casingColourVar}; stroke-width: {enc.alert
				.casingWidth}; stroke-opacity: {enc.alert.casingOpacity};"
		/>
	{/if}

	<!-- The line. Mass owns width + colour; TTL owns the dash. No endpoint arrowhead:
	     direction is a → arrow at the midpoint (below). -->
	<BaseEdge
		path={geom.path}
		style="stroke: {stroke}; stroke-width: {thickness}; stroke-linecap: round; {enc.ttl
			.dashArray
			? `stroke-dasharray: ${enc.ttl.dashArray};`
			: ''}"
	/>

	<!-- Direction arrow: a filled triangle on a dark backing disc at the MIDPOINT,
	     rotated to lie along the line pointing toward the K162 end. Absent when
	     direction is undetermined. Drawn as plain SVG in the edge's own group (NOT an
	     HTML EdgeLabel portal) so it sits in the same layer as the line, takes no
	     pointer events, and never masks the hover tooltip below — getting out of
	     svelte-flow's way instead of overriding its label wrapper. The disc fades from
	     a dark core to transparent so it lifts the glyph off the stroke it rides on
	     without printing a hard ring where it floats beside the line. -->
	{#if namedEnd != null}
		<g
			class="dir-arrow"
			transform="translate({geom.labelX} {geom.labelY}) rotate({dirAngle})"
			aria-hidden="true"
		>
			<!-- Backing disc as two stacked circles (a faint wide halo + a darker core)
			     rather than a radial-gradient — self-contained per edge, so no shared
			     <defs> id to manage. -->
			<circle r="11" fill="var(--space-950)" opacity="0.55" />
			<circle r="7.5" fill="var(--space-950)" />
			<!-- Triangle pointing +x; the group rotation aims it down-line. -->
			<path class="dir-tip" d="M-5 -5.5 L6.5 0 L-5 5.5 Z" />
		</g>
	{/if}

	<!-- Hover hit-path: a wide, transparent stroke over the line carrying the native
	     <title> tooltip, so hovering anywhere along the connection surfaces its status
	     (the tooltip lived on the deleted glyph before; it rides the line now). Kept
	     LAST so it is topmost — the hover falls to it (finger cursor + tooltip) even at
	     the midpoint where the decorative arrow sits. Width floors at a comfortable
	     hover target regardless of the mass-thin line. -->
	<path
		class="edge-hit"
		d={geom.path}
		fill="none"
		stroke="transparent"
		stroke-width={Math.max(thickness, 14)}
	>
		<title>{connTitle}</title>
	</path>

	<!-- Centre text label. A VISIBLE pill INHERITS the same hover tooltip as the line
	     (so the connection status shows whether or not the labels are enabled, and on
	     the pill itself); it is nudged off-line when the arrow shares the midpoint. When
	     the label is only carrying sr-only TTL text (no visible pill on an alert edge),
	     it gets `label-inert` → pointer-events:none, so its invisible div never steals
	     the line hover at the midpoint (the hand-cursor / lost-tooltip bug) — the line
	     beneath owns that pixel and shows the tooltip there. -->
	{#if showLabel}
		{@const nudge = labelVisible && namedEnd != null ? LABEL_NUDGE : 0}
		<EdgeLabel
			x={geom.labelX + geom.perpX * nudge}
			y={geom.labelY + geom.perpY * nudge}
			transparent
			class={labelVisible ? undefined : 'label-inert'}
		>
			<ConnectionEdgeLabel
				wh_type={d.wh_type}
				mass={d.mass}
				ttlBucket={enc.ttlBucket}
				alertLevel={enc.alert.level}
				showMass={d.showMass ?? true}
				showWhType={d.showWhType ?? true}
				title={connTitle}
			/>
		</EdgeLabel>
	{/if}

	<!-- Sig endpoint labels: which signature in each system leads to this hole.
	     Svelte Flow supports multiple <EdgeLabel>s per edge — these two sit near
	     the endpoints, the type/mass label sits at the midpoint above. Each pill
	     INHERITS the connection's hover tooltip (title), so hovering it surfaces the
	     same status as the line. -->
	{#if d.sig_a}
		<EdgeLabel x={geom.sigSourceX} y={geom.sigSourceY} transparent>
			<span class="sig-endpoint" title={connTitle}>{sigLabel(d.sig_a)}</span>
		</EdgeLabel>
	{/if}
	{#if d.sig_b}
		<EdgeLabel x={geom.sigTargetX} y={geom.sigTargetY} transparent>
			<span class="sig-endpoint" title={connTitle}>{sigLabel(d.sig_b)}</span>
		</EdgeLabel>
	{/if}
{/if}

<style>
	/* The alert casing breathes (spec §5): animate stroke-opacity AND stroke-width
	   together so the swell reads as an inhale, not a blink. Only the halo moves;
	   the line, dash, and badge stay still. Urgency scales by depth + rate, not
	   franticness — imminent stays at 2.8s so it doesn't strobe.

	   The keyframes oscillate AROUND the resting (inline-style) values, with the
	   0%/100% trough set to roughly the static midpoint so that when the global
	   reduced-motion rule freezes the animation at 0% (app.css), the halo holds a
	   sensible mid-breath weight instead of collapsing to nothing (spec §6). */
	:global(.edge-casing) {
		fill: none;
	}
	/* The invisible hover hit-path opts back INTO pointer events (the edge layer sets
	   none) so its <title> tooltip fires on hover; cursor hints it is interactive. */
	.edge-hit {
		pointer-events: stroke;
		cursor: pointer;
	}
	/* Informational EdgeLabels (the sr-only-only centre label on alert edges + the sig
	   pills) must NOT intercept the line hover where their wrapper boxes overlap the
	   stroke — that was the hand-cursor / lost-tooltip bug (invisible div parked on the
	   midpoint). They are never drag/click targets, so opt their wrappers out of pointer
	   events. SvelteFlow hardcodes `pointer-events: all` inline on every EdgeLabel
	   wrapper, so overriding it needs !important. (The VISIBLE centre pill keeps its
	   default `all` — it is fine for a shown pill to be hoverable, and it is nudged
	   off-line away from the stroke anyway.) */
	:global(.svelte-flow__edge-label.label-inert) {
		pointer-events: none !important;
	}
	/* The midpoint direction arrow is SVG in the edge group (not an HTML EdgeLabel), so
	   it takes no pointer events and never masks the hover tooltip on the hit-path drawn
	   after it. A sky drop-shadow gives the tip its own halo so it out-weighs the stroke
	   it rides on, either way. */
	.dir-arrow {
		pointer-events: none;
		filter: drop-shadow(0 0 2px rgb(2 6 23 / 0.9));
	}
	.dir-tip {
		fill: var(--sky);
	}
	:global(.edge-casing.halo-amber) {
		animation: breathe-soft 3.4s ease-in-out infinite;
	}
	:global(.edge-casing.halo-red) {
		animation: breathe-deep 2.8s ease-in-out infinite;
	}
	@keyframes breathe-soft {
		0%,
		100% {
			stroke-opacity: 0.1;
			stroke-width: 9;
		}
		50% {
			stroke-opacity: 0.22;
			stroke-width: 13;
		}
	}
	@keyframes breathe-deep {
		0%,
		100% {
			stroke-opacity: 0.12;
			stroke-width: 11;
		}
		50% {
			/* A wide swing on the richer halo red so the pulse clearly reads as an
			   alarm — deeper trough, brighter + fatter peak than the amber breath. */
			stroke-opacity: 0.5;
			stroke-width: 22;
		}
	}

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
