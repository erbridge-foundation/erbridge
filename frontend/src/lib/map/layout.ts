/**
 * Graph layout seed — ranked tree layout via dagre (@dagrejs/dagre).
 *
 * `layoutSeed(graph, tab, dir)` is a PURE function: graph → positions. It is the
 * FLOOR the live positions sit on (a node's first position; drags/ripples then own
 * it). The graph carries no coordinates; this is where they come from.
 *
 * Dagre does the heavy lifting — rank assignment (BFS-equivalent longest/shortest
 * path), crossing reduction, and proper node coordinate assignment (the
 * Brandes–Köpf style pass that centres parents over their children and keeps
 * sibling subtrees from overlapping ACROSS depths). We previously hand-rolled a
 * tidy-tree for this and kept hitting its limits (parents decoupling from children,
 * shallow and deep branches collapsing onto the same cross-line); dagre solves that
 * class of problem properly, so the canvas reads as a clean layered chain.
 *
 * What stays bespoke around dagre:
 *   - REACHABILITY + the gutter (ROOTED tab): only systems reachable from the tab root
 *     are fed to dagre; unreached nodes (a hand-added ghost, a disconnected fragment)
 *     are parked in a visible gutter beside the ranked chain (see `placeGutter`).
 *     Existence is a pure function of the graph, never of placement.
 *   - COMPONENT PACKING (root-less `*` tab): dagre lays a single rooted chain well, but
 *     a wildcard tab is a FOREST of several disconnected components (the DEEP chain, the
 *     Home chain, a lone system…). Feeding the whole disconnected graph to one dagre
 *     pass laid one component and shredded the rest into a flat 1-D gutter line. Instead
 *     we split `present` into connected components, dagre EACH on its own, then row/grid
 *     PACK the laid components by bounding box (see `components` + `packComponents`).
 *   - DIRECTION: dagre's `rankdir` handles LR/RL/TB/BT directly.
 *   - the `spacing` preference: scales dagre's cross-axis `nodesep` so a busy fan
 *     (e.g. a system with several dangling stubs) spreads apart.
 *
 * Incremental single-node placement on an SSE add (`place-incoming.ts`) and session
 * drag persistence are separate — only the whole-map (re)seed lives here.
 */

import dagre from '@dagrejs/dagre';
import type { CombinedGraph, LayoutDirection, Positions, System, Tab } from './types';

// Rank-axis spacing (between depth columns/rows) and the BASE cross-axis spacing
// (between siblings within a rank). `spacing` scales the cross axis. Node size is
// fed to dagre so it never overlaps boxes. These are tuned tight so the chain reads
// dense (closer to the EVE/Wanderer reference look) while staying crossing-free in
// BOTH directions — measured on the DEEP tab in LR and TB, since the map is used both
// ways. Looser values (the old 120/70) left dagre's output spread out with low fill.
const RANK_SEP = 90;
const NODE_SEP = 40;
const NODE_W = 130;
const NODE_H = 48;
/** Gutter sits to the left of (LR) / above (TB) the root column, far enough that
 *  unreached nodes don't collide with the ranked chain (ROOTED tab only). */
const GUTTER_GAP = 320;
const GUTTER_STEP = 150;
/** Forest packing (root-less `*` tab): gap between packed components (within a row and
 *  between rows) and the running width a row fills before wrapping. ROW_TARGET is a
 *  soft cap — a single component wider than it still gets its own row — tuned so the
 *  wildcard tab packs into a roughly square grid rather than one long strip. */
const PACK_GAP = 160;
const ROW_TARGET = 2200;

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
 * reachable from the seeds. The key set is the reachable set (used for the gutter
 * split and `renderableSystems`); the rank values let the dagre feed direct every
 * edge from the LOWER-rank endpoint to the higher one, so edges all flow outward
 * from the root and dagre lays a clean layered tree instead of routing back-edges
 * (authored a→b in the wrong order) as long sweeping curves. Deterministic.
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

/** A laid-out component: its node positions (dagre centre coords) and the bounding
 *  box of those nodes (node CENTRES, so packing adds NODE_W/NODE_H padding itself). */
type LaidComponent = { pos: Positions; minX: number; minY: number; maxX: number; maxY: number };

/**
 * Lay out ONE connected component with dagre and return its positions + bbox. This is
 * the per-component engine: the dagre setup (network-simplex, RANK_SEP/NODE_SEP, edges
 * directed lower-rank→higher-rank away from the component's own root) over a single
 * component's id set. `compRank` is the BFS rank within this component (seeded from the
 * component's most-natural root) used only to orient edges so dagre lays a clean tree.
 */
function layoutComponent(
	graph: CombinedGraph,
	ids: string[],
	compRank: Map<string, number>,
	dir: LayoutDirection,
	spacing: number
): LaidComponent {
	const idSet = new Set(ids);
	const g = new dagre.graphlib.Graph({ multigraph: false, compound: false });
	g.setGraph({
		rankdir: dir,
		ranksep: RANK_SEP,
		nodesep: NODE_SEP * spacing,
		ranker: 'network-simplex',
		marginx: 0,
		marginy: 0
	});
	g.setDefaultEdgeLabel(() => ({}));

	// Keep graph.systems order for deterministic dagre input.
	for (const s of graph.systems) if (idSet.has(s.id)) g.setNode(s.id, { width: NODE_W, height: NODE_H });

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
 * Row/grid pack laid components into `out`. Components are placed left→right, wrapping
 * to a new row once a row's running width would exceed `ROW_TARGET`; each row's height
 * is its tallest component. A fixed `PACK_GAP` separates components within a row and
 * between rows. Caller passes components already SORTED (largest first) so the big
 * chains lead and lone systems trail — deterministic. Each component's nodes are
 * translated so its bbox top-left lands at the packed offset.
 */
function packComponents(laid: LaidComponent[], out: Positions): void {
	let rowX = 0;
	let rowY = 0;
	let rowHeight = 0;
	for (const comp of laid) {
		const w = comp.maxX - comp.minX + NODE_W;
		const h = comp.maxY - comp.minY + NODE_H;
		// Wrap to a new row when this component would push the row past the target width
		// (but never wrap an empty row — always place at least one component per row).
		if (rowX > 0 && rowX + w > ROW_TARGET) {
			rowY += rowHeight + PACK_GAP;
			rowX = 0;
			rowHeight = 0;
		}
		// Translate component so its bbox min lands at (rowX, rowY).
		const dx = rowX - comp.minX;
		const dy = rowY - comp.minY;
		for (const [id, p] of Object.entries(comp.pos)) out[id] = { x: p.x + dx, y: p.y + dy };
		rowX += w + PACK_GAP;
		rowHeight = Math.max(rowHeight, h);
	}
}

/**
 * Seed positions for the systems of `graph` as viewed through `tab`, in the given
 * direction. The caller decides which systems are *present* (reachable + ghosts) and
 * passes them; every present system gets a position. A ROOTED tab is dagre-laid from
 * its root with unreached nodes parked in the gutter; the root-less `*` tab is a forest
 * — each connected component is dagre-laid on its own and the components are row/grid
 * packed (see the module doc) so no component collapses into a flat gutter line.
 */
export function layoutSeed(
	graph: CombinedGraph,
	tab: Tab,
	dir: LayoutDirection,
	present: Set<string>,
	/** Cross-axis spacing multiplier (a user "node spacing" preference). Scales
	 *  dagre's `nodesep` so siblings/fans spread apart; 1 = the compact default. */
	spacing = 1
): Positions {
	const adj = buildAdjacency(graph, present);
	const order = graph.systems.filter((s) => present.has(s.id)).map((s) => s.id);
	const out: Positions = {};

	// ROOT-LESS `*` tab: a forest. Lay each connected component on its own and row/grid
	// pack them, so no component shreds into a flat gutter line. Dagre returns CENTRE
	// coords; packing keeps that convention (every node shares it across the map).
	if (tab.isWildcard || !tab.root) {
		// Systems flagged `root` anchor their component's tree, so a component lays out
		// the SAME way on `*` as on its own rooted tab (the curated anchor, not the
		// degree heuristic — which can pick a different node and re-tangle the tree).
		const rootFlagged = new Set(
			graph.systems.filter((s) => s.flags?.includes('root')).map((s) => s.id)
		);
		const comps = components(adj, order);
		// Largest first (node count, ties by first id) so the big chains lead the grid
		// and lone systems trail — deterministic.
		comps.sort((a, b) => b.length - a.length || (a[0] < b[0] ? -1 : 1));
		const laid = comps.map((ids) => {
			// Root each component at its flagged-root system if it has one; otherwise the
			// most-connected hub (ties by id). Edges are oriented away from that root.
			const root = pickComponentRoot(adj, ids, rootFlagged);
			const compRank = reachableFrom(adj, [root]);
			return layoutComponent(graph, ids, compRank, dir, spacing);
		});
		packComponents(laid, out);
		return out;
	}

	// ROOTED tab: dagre the systems reachable from the root; park genuinely-unreached
	// present nodes (a ghost, a detached fragment) in the gutter.
	const rank = reachableFrom(adj, [tab.root]);
	const ranked = graph.systems.filter((s) => present.has(s.id) && rank.has(s.id)).map((s) => s.id);
	const gutter = graph.systems.filter((s) => present.has(s.id) && !rank.has(s.id));

	if (ranked.length > 0) {
		// Dagre returns each node's CENTRE; svelte-flow positions are top-left, but the
		// proto consistently treats these seeds as a coordinate space the whole map
		// shares, so centre coords are fine (every node uses the same convention).
		const comp = layoutComponent(graph, ranked, rank, dir, spacing);
		Object.assign(out, comp.pos);
	}

	placeGutter(dir, gutter.map((s) => s.id), out);
	return out;
}

/** Root for a component: a `root`-flagged system if the component contains one (the
 *  curated anchor — keeps the tree shape identical to that system's own rooted tab),
 *  else the most-connected hub via {@link pickRoot}. Among multiple flagged roots in
 *  one component, the lowest id wins (deterministic). */
function pickComponentRoot(
	adj: Map<string, string[]>,
	ids: string[],
	rootFlagged: Set<string>
): string {
	let flagged: string | undefined;
	for (const id of ids) {
		if (rootFlagged.has(id) && (flagged === undefined || id < flagged)) flagged = id;
	}
	return flagged ?? pickRoot(adj, ids);
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

/**
 * Park unreached nodes in a visible gutter: a column before the root column (LR),
 * a row above it (TB). They render normally but sit apart so an unreached ghost
 * reads as unreached.
 */
function placeGutter(dir: LayoutDirection, gutter: string[], out: Positions): void {
	gutter.forEach((id, i) => {
		switch (dir) {
			// Park the gutter on the side OPPOSITE the rank flow so unreached nodes sit
			// clear of the ranked chain.
			case 'LR':
				out[id] = { x: -GUTTER_GAP, y: i * GUTTER_STEP };
				break;
			case 'RL':
				out[id] = { x: GUTTER_GAP, y: i * GUTTER_STEP };
				break;
			case 'TB':
				out[id] = { x: i * GUTTER_STEP, y: -GUTTER_GAP };
				break;
			case 'BT':
				out[id] = { x: i * GUTTER_STEP, y: GUTTER_GAP };
				break;
		}
	});
}

/**
 * The systems that RENDER for a tab: those reachable from `tab.root` over live
 * connections, plus any ghosts (which park in the gutter). The wildcard tab renders
 * every system. Existence is a pure function of the graph — NEVER of placement.
 * Returned as a Set of ids for layout/reconcile to consume.
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
