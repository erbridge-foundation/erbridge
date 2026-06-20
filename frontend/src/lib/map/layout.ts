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
 *   - REACHABILITY + the gutter: only systems reachable from the tab root are fed to
 *     dagre; unreached nodes (a hand-added ghost, a disconnected fragment) are parked
 *     in a visible gutter beside the ranked chain (see `placeGutter`). Existence is a
 *     pure function of the graph, never of placement.
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
 *  unreached nodes don't collide with the ranked chain. */
const GUTTER_GAP = 320;
const GUTTER_STEP = 150;

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

/**
 * Seed positions for the systems of `graph` as viewed through `tab`, in the given
 * direction. The caller decides which systems are *present* (reachable + ghosts) and
 * passes them; everything in `present` gets a position — ranked via dagre if
 * reachable from the root, parked in the gutter otherwise.
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

	// A normal tab seeds from its single root; the wildcard / root-less tab has no
	// anchor, so seed from the first present system (stable by `graph.systems` order)
	// to keep the layout connected-looking and deterministic.
	const roots =
		!tab.isWildcard && tab.root
			? [tab.root]
			: graph.systems
					.filter((s) => present.has(s.id))
					.slice(0, 1)
					.map((s) => s.id);

	const rank = reachableFrom(adj, roots);
	// Keep `graph.systems` order so dagre's input (and thus output) is deterministic.
	const ranked = graph.systems.filter((s) => present.has(s.id) && rank.has(s.id));
	const gutter = graph.systems.filter((s) => present.has(s.id) && !rank.has(s.id));

	const out: Positions = {};

	if (ranked.length > 0) {
		const g = new dagre.graphlib.Graph({ multigraph: false, compound: false });
		g.setGraph({
			rankdir: dir,
			ranksep: RANK_SEP,
			nodesep: NODE_SEP * spacing,
			// `network-simplex` gives dagre's best rank assignment + crossing reduction.
			// The DEEP chain is a pure tree (no same-rank cross-links), so this produces a
			// crossing-free ordering where `tight-tree` left wide fan-out hubs tangled.
			ranker: 'network-simplex',
			marginx: 0,
			marginy: 0
		});
		g.setDefaultEdgeLabel(() => ({}));

		for (const s of ranked) g.setNode(s.id, { width: NODE_W, height: NODE_H });
		// Edges among reachable nodes only, DIRECTED from the lower-rank endpoint to the
		// higher one (i.e. away from the root) — regardless of the fixture's a/b order.
		// Feeding back-edges (a authored farther from the root than b) made dagre reverse
		// + route them as long sweeping curves that crossed the whole layout (the DEEP
		// hub tangle); ranking them outward keeps the chain a clean layered tree. A
		// same-rank cross-link is ordered by id for determinism. Added once (non-multigraph
		// dedupes by endpoints).
		for (const c of graph.connections) {
			const a = c.a.system;
			const b = c.b.system;
			if (!rank.has(a) || !rank.has(b) || a === b) continue;
			const ra = rank.get(a)!;
			const rb = rank.get(b)!;
			// Lower rank → higher rank; ties broken by id so it's deterministic.
			const [from, to] = ra < rb || (ra === rb && a < b) ? [a, b] : [b, a];
			if (!g.hasEdge(from, to) && !g.hasEdge(to, from)) {
				g.setEdge(from, to);
			}
		}

		dagre.layout(g);

		// Dagre returns each node's CENTRE; svelte-flow positions are top-left, but the
		// proto consistently treats these seeds as a coordinate space the whole map
		// shares, so centre coords are fine (every node uses the same convention). Keep
		// them as-is for parity with the previous output's relative geometry.
		for (const id of g.nodes()) {
			const n = g.node(id);
			if (n) out[id] = { x: n.x, y: n.y };
		}
	}

	placeGutter(dir, gutter.map((s) => s.id), out);
	return out;
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
