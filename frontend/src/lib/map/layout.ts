/**
 * Graph layout seed — TWO selectable engines behind one pure contract.
 *
 * `layoutSeed(graph, tab, dir, present, crossSpacing, algorithm, rankSpacing)` is a PURE function:
 * graph → positions. It is the FLOOR the live positions sit on (a node's first
 * position; drags/ripples then own it). The graph carries no coordinates; this is
 * where they come from. `algorithm` selects the engine (a user preference); both
 * honour the same contract (a crossing-free ranked forest, tab-root anchoring, the
 * two spacing multipliers [cross-axis + rank-axis], LR/RL/TB/BT) and differ only in
 * feel:
 *
 *   - `tidy-tree` (the corp's Wanderer look) — a leaf-first tidy tree where a leaf
 *     HUGS its parent and claims no reserved cross-axis band, with node-size-aware
 *     rank columns. Ports the tested core of the Go layout
 *     (zz-ref/wanderer-layout/layout.go — `calculateTreePositions`): a leaf takes the
 *     next free cross slot; a parent starts its children level with itself then
 *     re-centres on the midpoint of its first & last child (grid-snapped). See
 *     `tidyTreeSeed`.
 *   - `dagre` (@dagrejs/dagre) — dagre's layered Sugiyama layout: every node centred
 *     in a RESERVED band balanced against its siblings (an even "org-chart" feel,
 *     more whitespace around leaves). See `dagreSeed`.
 *
 * FOREST model (both engines, every tab). `present` is split into connected
 * components; a component roots at a tab-root system it contains (any of the
 * client-side tab roots — see {@link Tab.root}), else the most-connected hub, so a
 * chain lays out the same on every tab incl. the root-less `*` tab. The
 * tidy-tree engine stacks components down the cross axis (primary first, all oriented
 * the same way); the dagre engine row/grid PACKS them by bounding box. A rooted tab's
 * genuinely-unreached nodes (a ghost) park in a gutter (dagre) / a trailing satellite
 * (tidy-tree).
 *
 * Incremental single-node placement on an SSE add (`place-incoming.ts`) and session
 * drag persistence are separate — only the whole-map (re)seed lives here.
 */

import dagre from '@dagrejs/dagre';
import type {
	CombinedGraph,
	LayoutAlgorithm,
	LayoutDirection,
	Positions,
	System,
	Tab
} from './types';

// BASE rank-axis GAP (empty space between one rank's column and the next) and the BASE
// cross-axis step (between siblings within a rank). Each has its OWN user multiplier:
// `crossSpacing` scales the cross step, `rankSpacing` scales the rank gap — so the two
// axes spread independently. Unlike dagre (which we fed per-node widths), this tidy tree
// historically treated the rank step as a raw centre-to-centre constant that ignored node
// extent — so wide nodes crowded their neighbours. The rank axis is now NODE-SIZE-AWARE:
// each rank's column sits a half-width + (RANK_GAP × rankSpacing) + half-width from the
// previous (see `rankOffsets`), so RANK_GAP is true empty space, tuned tight for BOTH
// directions.
const RANK_GAP = 70;
const CROSS_SEP = 70;
// Parent re-centre snaps to this grid (mirrors the Go layout's gridSize) so equal
// inputs give stable, non-jittery parent positions.
const GRID = 15;
/** Gap between a satellite component and the previous one. */
const GUTTER_GAP = 160;

/** The set of systems that anchor a tab — the CLIENT-SIDE tab roots (see {@link
 *  Tab.root}). A component containing one of these roots its tree there, so a chain
 *  lays out the same on every tab (incl. the root-less `*` tab, which has no `tab.root`
 *  of its own). This is the curation signal read from its true home — the tabs — rather
 *  than duplicated onto the shared System (a root is not shared intel; see SystemFlag). */
function tabRootSet(graph: CombinedGraph): Set<string> {
	return new Set(
		graph.tabs.filter((t) => !t.isWildcard && t.root).map((t) => t.root)
	);
}

// ── Node-width estimate (item a) ─────────────────────────────────────────────
// SystemNodes are NOT fixed-width: min-width 110px + padding, content-driven. The
// layout has no DOM, so it ESTIMATES width from the system's data — a pure, deterministic
// pre-render seed. Each rank's column step uses the MAX estimated width in that rank, so
// the layout is tight where nodes are narrow and roomy where they're wide (the root
// J172840, the named k-space systems Charmerout/Hurjafren).
const NODE_MIN_W = 110; // CSS min-width
const NODE_PAD_X = 19; // 0.6rem padding each side + 1px border ≈ 19px total
const CHAR_W = 6.2; // ~px per char of the name at 0.75rem 600-weight ui font
const CLASS_BADGE_W = 26; // the always-present class pill ("C5", "HS"…) + its gap
const ROOT_BADGE_W = 42; // the "ROOT" badge + gap (uppercase, letter-spaced)
const STATIC_BADGE_W = 26; // each static dest pill (they wrap, but a long row can widen)

/** Pure estimate of a SystemNode's rendered width in px, from its data alone. Used to
 *  size each rank's column step. No DOM; deterministic. A dangling stub renders a
 *  minimal `? → dest`, narrower than a real node, so it floors at NODE_MIN_W. `isRoot`
 *  (whether this system anchors a tab — a client-side fact, not on the System) adds the
 *  ROOT badge's width; the caller passes it from the tab-root set. */
function nodeWidth(s: System, isRoot: boolean): number {
	// Header content: class badge + name + optional root badge. (Ghost badge only on a
	// hand-added local node; statics wrap below the header but a wide static row can
	// still push the box out, so count the row width and take the max with the header.)
	const header =
		CLASS_BADGE_W + s.name.length * CHAR_W + (isRoot ? ROOT_BADGE_W : 0) + NODE_PAD_X;
	const statics = s.statics.length > 0 ? s.statics.length * STATIC_BADGE_W + NODE_PAD_X : 0;
	return Math.max(NODE_MIN_W, header, statics);
}

/** Adjacency over the rendered systems only (a connection to a system that isn't
 *  in `present` is ignored). Undirected: reachability and layout both walk a
 *  connection both ways. */
function buildAdjacency(graph: CombinedGraph, present: Set<string>): Map<string, string[]> {
	const adj = new Map<string, string[]>();
	for (const s of graph.systems) {
		if (present.has(s.id)) adj.set(s.id, []);
	}
	for (const c of graph.connections) {
		if (!present.has(c.a.system) || !present.has(c.b.system)) continue;
		adj.get(c.a.system)!.push(c.b.system);
		adj.get(c.b.system)!.push(c.a.system);
	}
	return adj;
}

/**
 * Reachability BFS: `id → rank` (min hop distance from the seed(s)) for every node
 * reachable from the seeds. Used by `renderableSystems` (the reachable set for a
 * normal tab). Deterministic.
 */
function reachableFrom(adj: Map<string, string[]>, roots: string[]): Map<string, number> {
	const rank = new Map<string, number>();
	const queue: string[] = [];
	for (const r of roots) {
		if (adj.has(r) && !rank.has(r)) {
			rank.set(r, 0);
			queue.push(r);
		}
	}
	let head = 0;
	while (head < queue.length) {
		const node = queue[head++];
		const here = rank.get(node)!;
		for (const next of adj.get(node)!) {
			if (!rank.has(next)) {
				rank.set(next, here + 1);
				queue.push(next);
			}
		}
	}
	return rank;
}

/** Connected components over `adj`, each a list of ids in BFS-discovery order. The
 *  outer order follows `order` (stable input order) so the result is deterministic. */
function components(adj: Map<string, string[]>, order: string[]): string[][] {
	const seen = new Set<string>();
	const out: string[][] = [];
	for (const start of order) {
		if (!adj.has(start) || seen.has(start)) continue;
		const comp: string[] = [];
		const queue = [start];
		seen.add(start);
		let head = 0;
		while (head < queue.length) {
			const node = queue[head++];
			comp.push(node);
			for (const next of adj.get(node)!) {
				if (!seen.has(next)) {
					seen.add(next);
					queue.push(next);
				}
			}
		}
		out.push(comp);
	}
	return out;
}

/** Root for a component: a tab-root system if the component contains one (the curated
 *  anchor — keeps the tree shape identical to that system's own rooted tab), else the
 *  most-connected hub via {@link pickRoot}. Among multiple tab-root systems in one
 *  component, the lowest id wins (deterministic). */
function pickComponentRoot(
	adj: Map<string, string[]>,
	ids: string[],
	rootAnchors: Set<string>
): string {
	let anchored: string | undefined;
	for (const id of ids) {
		if (rootAnchors.has(id) && (anchored === undefined || id < anchored)) anchored = id;
	}
	return anchored ?? pickRoot(adj, ids);
}

/** Root for a component without a designated root: the most-connected system (hub),
 *  ties broken by id for determinism. */
function pickRoot(adj: Map<string, string[]>, ids: string[]): string {
	let best = ids[0];
	let bestDeg = adj.get(best)?.length ?? 0;
	for (const id of ids) {
		const deg = adj.get(id)?.length ?? 0;
		if (deg > bestDeg || (deg === bestDeg && id < best)) {
			best = id;
			bestDeg = deg;
		}
	}
	return best;
}

/** A BFS spanning tree of a component, rooted at `root`: `parent → ordered children`.
 *  Non-tree links (a same-rank cross-link, an extra edge into a node) are simply not
 *  tree edges; they still render from the connection list, they just don't shape the
 *  tree. Children follow adjacency (insertion) order for a deterministic layout. */
function buildChildren(adj: Map<string, string[]>, root: string): Map<string, string[]> {
	const children = new Map<string, string[]>();
	const seen = new Set<string>([root]);
	const queue = [root];
	let head = 0;
	while (head < queue.length) {
		const node = queue[head++];
		for (const next of adj.get(node)!) {
			if (!seen.has(next)) {
				seen.add(next);
				if (!children.has(node)) children.set(node, []);
				children.get(node)!.push(next);
				queue.push(next);
			}
		}
	}
	return children;
}

/** Per-axis coordinate of one tidy tree: `id → { rank, cross }`. */
type TreePos = Map<string, { rank: number; cross: number }>;

/**
 * Leaf-first tidy-tree layout (ported from the Go `calculateTreePositions`). DFS from
 * `root`: a leaf takes the next free cross slot at its depth; a parent starts its
 * children level with itself (`nextCross[childLevel] = parentCross`), then is
 * re-centred on the midpoint of its first and last child, grid-snapped. `rank` is the
 * depth; `cross` is the packed cross-axis position. `root` ends up centred over its
 * subtree (the caller then translates it to the origin).
 */
function tidyTree(root: string, children: Map<string, string[]>, crossStep: number): TreePos {
	const pos: TreePos = new Map();
	const nextCross = new Map<number, number>();

	function calc(id: string, depth: number): number {
		const cross = nextCross.get(depth) ?? 0;
		pos.set(id, { rank: depth, cross });

		const kids = children.get(id) ?? [];
		if (kids.length === 0) {
			nextCross.set(depth, cross + crossStep);
			return crossStep;
		}

		// Children start at this parent's own cross, not after a prior sibling subtree.
		nextCross.set(depth + 1, cross);

		let totalHeight = 0;
		let firstCross = 0;
		let lastCross = 0;
		kids.forEach((kid, i) => {
			totalHeight += calc(kid, depth + 1);
			const kc = pos.get(kid)!.cross;
			if (i === 0) firstCross = kc;
			lastCross = kc;
		});

		// Re-centre the parent on the midpoint of its first & last child (grid-snapped).
		const centred = Math.round((firstCross + lastCross) / 2 / GRID) * GRID;
		pos.set(id, { rank: depth, cross: centred });

		nextCross.set(depth, cross + totalHeight);
		return totalHeight;
	}

	calc(root, 0);
	return pos;
}

/** Cross-axis span [min, max] of a tree's nodes (for satellite stacking). */
function crossSpan(pos: TreePos): { min: number; max: number } {
	let min = Infinity;
	let max = -Infinity;
	for (const { cross } of pos.values()) {
		if (cross < min) min = cross;
		if (cross > max) max = cross;
	}
	if (min === Infinity) return { min: 0, max: 0 };
	return { min, max };
}

/** Deepest rank in a tree (its rank-axis extent, in rank units not pixels). */
function maxRank(pos: TreePos): number {
	let m = 0;
	for (const { rank } of pos.values()) if (rank > m) m = rank;
	return m;
}

/**
 * Node-size-aware rank-axis offsets (item a): `rank → centre offset in px`. The widest
 * node in each rank sets that rank's column width; consecutive columns sit a half-width
 * + RANK_GAP + half-width apart, so a wide rank (the root, a long k-space name) pushes
 * its neighbours out exactly as far as it needs and no further. `widthOf(id)` is the
 * pure {@link nodeWidth} estimate. `rankGap` is the inter-column empty space (the base
 * RANK_GAP already scaled by the rank-spacing multiplier). Rank 0's centre is at its own
 * half-width so the tree starts flush at offset 0 on the rank axis. The deepest rank's
 * far edge is the tree's rank-axis EXTENT (returned as `extent` for satellite stacking). */
function rankOffsets(
	pos: TreePos,
	widthOf: (id: string) => number,
	rankGap: number
): {
	offset: Map<number, number>;
	extent: number;
} {
	const maxW = new Map<number, number>();
	for (const [id, { rank }] of pos) {
		const w = widthOf(id);
		if (w > (maxW.get(rank) ?? 0)) maxW.set(rank, w);
	}
	const deepest = maxRank(pos);
	const offset = new Map<number, number>();
	let edge = 0; // running far edge of the columns placed so far
	for (let r = 0; r <= deepest; r++) {
		const w = maxW.get(r) ?? NODE_MIN_W;
		offset.set(r, edge + w / 2); // centre of this column
		edge += w + rankGap; // advance past this column + the gap
	}
	// `edge` overshot by one rankGap after the last column; the true extent is up to the
	// last column's far edge.
	return { offset, extent: Math.max(0, edge - rankGap) };
}

/** Map a tree's (rank, cross) to absolute (x, y) for `dir`, shifting cross by
 *  `crossShift` (sibling/satellite stacking) and offsetting the rank axis by
 *  `rankBase + rankOff[rank]` (rankBase = 0 for the primary, the satellite offset
 *  otherwise; `rankOff` is the node-size-aware per-rank centre). `flip` mirrors the
 *  rank axis within the component (satellites grow the SAME visual direction as the
 *  primary — see the satellite stacking). Writes into `out`. */
function emit(
	pos: TreePos,
	dir: LayoutDirection,
	rankBase: number,
	rankOff: Map<number, number>,
	extent: number,
	flip: boolean,
	crossShift: number,
	out: Positions
): void {
	for (const [id, { rank, cross }] of pos) {
		// Rank-axis position: the column centre, optionally mirrored within [0, extent].
		const local = rankOff.get(rank) ?? 0;
		const r = rankBase + (flip ? extent - local : local);
		const c = cross + crossShift;
		switch (dir) {
			case 'LR':
				out[id] = { x: r, y: c };
				break;
			case 'RL':
				out[id] = { x: -r, y: c };
				break;
			case 'TB':
				out[id] = { y: r, x: c };
				break;
			case 'BT':
				out[id] = { y: -r, x: c };
				break;
		}
	}
}

/**
 * TIDY-TREE engine. Seed positions as a leaf-first tidy-tree forest: the present set is
 * split into connected components; the primary component (holding `tab.root` / a flagged
 * root, or the largest on the root-less `*` tab) leads, and the rest stack as satellites
 * along the CROSS axis below it — every component oriented the SAME way (root at rank 0,
 * leaves growing the same direction), so they read as peer trees, not mirror images.
 */
function tidyTreeSeed(
	graph: CombinedGraph,
	tab: Tab,
	dir: LayoutDirection,
	present: Set<string>,
	crossSpacing: number,
	rankSpacing: number
): Positions {
	const adj = buildAdjacency(graph, present);
	const crossStep = CROSS_SEP * crossSpacing;
	const rankGap = RANK_GAP * rankSpacing;
	const order = graph.systems.filter((s) => present.has(s.id)).map((s) => s.id);
	const comps = components(adj, order);

	const out: Positions = {};
	if (comps.length === 0) return out;

	// Tab-root systems anchor their component's tree, so a chain lays out the SAME on
	// every tab (incl. `*`) as on its own rooted tab. Anchors come from the client-side
	// tab roots, not the shared System (a root is not shared intel).
	const rootAnchors = tabRootSet(graph);
	// Per-id width estimate for the node-size-aware rank step (item a). A tab-root node
	// carries the ROOT badge, so feed that into the width estimate.
	const byId = new Map(graph.systems.map((s) => [s.id, s] as const));
	const widthOf = (id: string): number => {
		const s = byId.get(id);
		return s ? nodeWidth(s, rootAnchors.has(id)) : NODE_MIN_W;
	};

	// PRIMARY = the component holding tab.root (normal tab), else the largest (the
	// root-less `*` tab). Ties on size fall to the first in stable order.
	const rootId = !tab.isWildcard && tab.root ? tab.root : undefined;
	let primaryIdx = 0;
	if (rootId) {
		const idx = comps.findIndex((c) => c.includes(rootId));
		if (idx >= 0) primaryIdx = idx;
	} else {
		let bestSize = -1;
		comps.forEach((c, i) => {
			if (c.length > bestSize) {
				bestSize = c.length;
				primaryIdx = i;
			}
		});
	}

	// Order components: primary first, the rest in stable order. Each is laid as its own
	// tidy tree and stacked down the cross axis, all oriented identically (flip=false).
	const ordered = [comps[primaryIdx], ...comps.filter((_, i) => i !== primaryIdx)];
	let cursor = 0; // running cross-axis offset for the next component
	ordered.forEach((comp, i) => {
		// The primary honours this tab's root; a satellite uses a tab-root system it
		// contains or its hub. (The primary already preferred the tab root above.)
		const root =
			i === 0 && rootId && comp.includes(rootId) ? rootId : pickComponentRoot(adj, comp, rootAnchors);
		const pos = tidyTree(root, buildChildren(adj, root), crossStep);
		const { offset, extent } = rankOffsets(pos, widthOf, rankGap);
		const span = crossSpan(pos);
		// Stack along the cross axis: shift this component so its top edge sits at `cursor`.
		emit(pos, dir, 0, offset, extent, false, cursor - span.min, out);
		cursor += span.max - span.min + crossStep + GUTTER_GAP;
	});

	return out;
}

// ── DAGRE engine ─────────────────────────────────────────────────────────────
// dagre's layered Sugiyama layout. Per-component (so the root-less `*` tab is a forest,
// not a shredded gutter line); a rooted tab dagre's its root's reachable set with
// unreached nodes in a gutter. Tuned tight for BOTH directions (measured on DEEP LR+TB).
const DAGRE_RANK_SEP = 90;
const DAGRE_NODE_SEP = 40;
const DAGRE_NODE_W = 130;
const DAGRE_NODE_H = 48;
/** Gutter offset (rooted tab) for genuinely-unreached nodes (a ghost). */
const DAGRE_GUTTER_GAP = 320;
const DAGRE_GUTTER_STEP = 150;
/** Forest packing (root-less `*` tab): gap between packed components + the running row
 *  width before wrapping (soft cap → roughly square grid, not one long strip). */
const DAGRE_PACK_GAP = 160;
const DAGRE_ROW_TARGET = 2200;

/** A laid-out component: node positions (dagre centre coords) + their bounding box. */
type LaidComponent = { pos: Positions; minX: number; minY: number; maxX: number; maxY: number };

/**
 * Lay out ONE connected component with dagre and return its positions + bbox. The dagre
 * setup (network-simplex, RANK/NODE sep, edges directed lower-rank→higher-rank away from
 * the component's root via `compRank`) over a single component's id set.
 */
function layoutComponent(
	graph: CombinedGraph,
	ids: string[],
	compRank: Map<string, number>,
	dir: LayoutDirection,
	crossSpacing: number,
	rankSpacing: number
): LaidComponent {
	const idSet = new Set(ids);
	const g = new dagre.graphlib.Graph({ multigraph: false, compound: false });
	g.setGraph({
		rankdir: dir,
		// `ranksep` is the depth-to-depth (rank-axis) gap; `nodesep` the sibling (cross-
		// axis) gap. Each gets its own user multiplier so the axes spread independently.
		ranksep: DAGRE_RANK_SEP * rankSpacing,
		nodesep: DAGRE_NODE_SEP * crossSpacing,
		ranker: 'network-simplex',
		marginx: 0,
		marginy: 0
	});
	g.setDefaultEdgeLabel(() => ({}));

	// Keep graph.systems order for deterministic dagre input.
	for (const s of graph.systems)
		if (idSet.has(s.id)) g.setNode(s.id, { width: DAGRE_NODE_W, height: DAGRE_NODE_H });

	for (const c of graph.connections) {
		const a = c.a.system;
		const b = c.b.system;
		if (!idSet.has(a) || !idSet.has(b) || a === b) continue;
		const ra = compRank.get(a)!;
		const rb = compRank.get(b)!;
		const [from, to] = ra < rb || (ra === rb && a < b) ? [a, b] : [b, a];
		if (!g.hasEdge(from, to) && !g.hasEdge(to, from)) g.setEdge(from, to);
	}

	dagre.layout(g);

	const pos: Positions = {};
	let minX = Infinity,
		minY = Infinity,
		maxX = -Infinity,
		maxY = -Infinity;
	for (const id of g.nodes()) {
		const n = g.node(id);
		if (!n) continue;
		pos[id] = { x: n.x, y: n.y };
		minX = Math.min(minX, n.x);
		minY = Math.min(minY, n.y);
		maxX = Math.max(maxX, n.x);
		maxY = Math.max(maxY, n.y);
	}
	if (minX === Infinity) minX = minY = maxX = maxY = 0;
	return { pos, minX, minY, maxX, maxY };
}

/**
 * Row/grid pack laid components into `out`: placed left→right, wrapping to a new row when
 * the running width would exceed `DAGRE_ROW_TARGET`; row height = its tallest component;
 * `DAGRE_PACK_GAP` between components/rows. Caller passes components SORTED largest-first.
 */
function packComponents(laid: LaidComponent[], out: Positions): void {
	let rowX = 0;
	let rowY = 0;
	let rowHeight = 0;
	for (const comp of laid) {
		const w = comp.maxX - comp.minX + DAGRE_NODE_W;
		const h = comp.maxY - comp.minY + DAGRE_NODE_H;
		// Wrap to a new row when this component would push the row past the target width
		// (but never wrap an empty row — always place at least one component per row).
		if (rowX > 0 && rowX + w > DAGRE_ROW_TARGET) {
			rowY += rowHeight + DAGRE_PACK_GAP;
			rowX = 0;
			rowHeight = 0;
		}
		const dx = rowX - comp.minX;
		const dy = rowY - comp.minY;
		for (const [id, p] of Object.entries(comp.pos)) out[id] = { x: p.x + dx, y: p.y + dy };
		rowX += w + DAGRE_PACK_GAP;
		rowHeight = Math.max(rowHeight, h);
	}
}

/**
 * Park unreached nodes (a ghost, a detached fragment) in a gutter on the side opposite
 * the rank flow: a column before the root column (LR), a row above it (TB).
 */
function placeGutter(dir: LayoutDirection, gutter: string[], out: Positions): void {
	gutter.forEach((id, i) => {
		switch (dir) {
			case 'LR':
				out[id] = { x: -DAGRE_GUTTER_GAP, y: i * DAGRE_GUTTER_STEP };
				break;
			case 'RL':
				out[id] = { x: DAGRE_GUTTER_GAP, y: i * DAGRE_GUTTER_STEP };
				break;
			case 'TB':
				out[id] = { x: i * DAGRE_GUTTER_STEP, y: -DAGRE_GUTTER_GAP };
				break;
			case 'BT':
				out[id] = { x: i * DAGRE_GUTTER_STEP, y: DAGRE_GUTTER_GAP };
				break;
		}
	});
}

/**
 * DAGRE engine. A rooted tab dagre's the systems reachable from `tab.root` (unreached →
 * gutter); the root-less `*` tab splits into components, dagre's each, and row/grid packs
 * them (so no component shreds into a flat gutter line). Components root at a tab-root
 * system they contain, so a chain lays out the same on `*` as on its own tab.
 */
function dagreSeed(
	graph: CombinedGraph,
	tab: Tab,
	dir: LayoutDirection,
	present: Set<string>,
	crossSpacing: number,
	rankSpacing: number
): Positions {
	const adj = buildAdjacency(graph, present);
	const order = graph.systems.filter((s) => present.has(s.id)).map((s) => s.id);
	const out: Positions = {};

	if (tab.isWildcard || !tab.root) {
		const rootAnchors = tabRootSet(graph);
		const comps = components(adj, order);
		// Largest first so the big chains lead the grid and lone systems trail.
		comps.sort((a, b) => b.length - a.length || (a[0] < b[0] ? -1 : 1));
		const laid = comps.map((ids) => {
			const root = pickComponentRoot(adj, ids, rootAnchors);
			const compRank = reachableFrom(adj, [root]);
			return layoutComponent(graph, ids, compRank, dir, crossSpacing, rankSpacing);
		});
		packComponents(laid, out);
		return out;
	}

	const rank = reachableFrom(adj, [tab.root]);
	const ranked = graph.systems.filter((s) => present.has(s.id) && rank.has(s.id)).map((s) => s.id);
	const gutter = graph.systems.filter((s) => present.has(s.id) && !rank.has(s.id));

	if (ranked.length > 0) {
		const comp = layoutComponent(graph, ranked, rank, dir, crossSpacing, rankSpacing);
		Object.assign(out, comp.pos);
	}

	placeGutter(dir, gutter.map((s) => s.id), out);
	return out;
}

// ── Dispatcher ───────────────────────────────────────────────────────────────
/**
 * Seed positions for the systems of `graph` as viewed through `tab`, in `dir`, with the
 * chosen `algorithm` engine. The caller decides which systems are *present* (reachable +
 * ghosts); every present system gets a position. There are TWO independent spacing
 * multipliers (both user prefs): `crossSpacing` spreads siblings within a rank (the
 * cross axis — vertical in the default LR layout), `rankSpacing` spreads depth levels
 * apart (the rank axis — horizontal in LR). `rankSpacing` is the LAST param so existing
 * 6-arg callers (cross + algorithm only) keep working unchanged. `algorithm` is the
 * layout-engine preference.
 */
export function layoutSeed(
	graph: CombinedGraph,
	tab: Tab,
	dir: LayoutDirection,
	present: Set<string>,
	crossSpacing = 1,
	algorithm: LayoutAlgorithm = 'dagre',
	rankSpacing = 1
): Positions {
	return algorithm === 'tidy-tree'
		? tidyTreeSeed(graph, tab, dir, present, crossSpacing, rankSpacing)
		: dagreSeed(graph, tab, dir, present, crossSpacing, rankSpacing);
}

/**
 * The systems that RENDER for a tab: those reachable from `tab.root` over live
 * connections, plus any ghosts (which become their own gutter satellite). The
 * wildcard tab renders every system. Existence is a pure function of the graph —
 * NEVER of placement. Returned as a Set of ids for layout/reconcile to consume.
 */
export function renderableSystems(graph: CombinedGraph, tab: Tab, ghosts: System[]): Set<string> {
	const ids = new Set<string>();

	if (tab.isWildcard) {
		for (const s of graph.systems) ids.add(s.id);
	} else {
		const present = new Set(graph.systems.map((s) => s.id));
		const adj = buildAdjacency(graph, present);
		for (const id of reachableFrom(adj, [tab.root]).keys()) ids.add(id);
	}

	// Ghosts always render (they're what the user added); they park in the gutter
	// unless a live connection already reaches them.
	for (const g of ghosts) ids.add(g.id);
	return ids;
}
