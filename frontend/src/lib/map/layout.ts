/**
 * Hand-rolled BFS layout seed (Fork 2 — no layout library).
 *
 * `layoutSeed(graph, tab, dir)` is a PURE function: graph → positions. It is the
 * FLOOR the placement overlay sits on (`pos[id] = saved[id] ?? seed[id]`). The
 * graph carries no coordinates; this is where they come from.
 *
 *   rank(node)    = min BFS hop distance from `tab.root`
 *   sibling(node) = index among nodes sharing a rank, ordered by the barycenter
 *                   heuristic (see `orderRanks`) to reduce edge crossings — a
 *                   child seats beside its parent rather than in raw input order.
 *
 *   LR     : x = rank * DX,    y = sibling * DY
 *   TB     : x = sibling * DX, y = rank * DY
 *
 * Systems not reachable from the root (a ghost the user added that no live
 * connection reaches, or a disconnected fragment) are PARKED in a gutter rank so
 * they stay visible but read as clearly-unreached. The wildcard tab has no root,
 * so *every* system parks-or-ranks from a synthetic seed: we rank from the whole
 * node set's first element to keep it deterministic and connected-looking.
 */

import type { CombinedGraph, LayoutDirection, Positions, System, Tab, XY } from './types';

// Spacing between ranks / siblings. Tuned for the wireframe's node size; these
// are seed positions only — the user re-drags freely afterwards.
const DX = 260;
const DY = 150;
/** Gutter sits to the left of (LR) / above (TB) the root column at rank -1-ish,
 *  far enough that unreached nodes don't collide with ranked ones. */
const GUTTER_GAP = 320;

/** Adjacency over the rendered systems only (a connection to a system that isn't
 *  in `systems` — e.g. filtered out — is ignored). Undirected: reachability and
 *  layout both walk a connection both ways. */
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
 * BFS: rank = min hop from the seed(s). Returns a map of id → rank for every
 * reachable node; unreached nodes are absent (parked later). Takes a list because
 * it's a multi-source BFS, but a normal tab passes exactly one root (the wildcard
 * passes a single synthetic seed). Deterministic: seeds get rank 0 in list order;
 * the queue preserves insertion order so equal-rank discovery is stable.
 */
function bfsRanks(adj: Map<string, string[]>, roots: string[]): Map<string, number> {
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

/** Place one rank's siblings deterministically (sibling index from orderRanks). */
function placeRank(dir: LayoutDirection, rank: number, siblings: string[], out: Positions): void {
	siblings.forEach((id, sibling) => {
		out[id] = positionFor(dir, rank, sibling);
	});
}

function positionFor(dir: LayoutDirection, rank: number, sibling: number): XY {
	switch (dir) {
		case 'LR':
			return { x: rank * DX, y: sibling * DY };
		case 'RL':
			// Mirror of LR: ranks grow leftwards (roots on the right). `|| 0`
			// normalises a rank-0 `-0` to `+0`.
			return { x: -rank * DX || 0, y: sibling * DY };
		case 'TB':
			return { x: sibling * DX, y: rank * DY };
		case 'BT':
			// Mirror of TB: ranks grow upwards (roots at the bottom).
			return { x: sibling * DX, y: -rank * DY || 0 };
	}
}

/**
 * Seed positions for the systems of `graph` as viewed through `tab`, in the
 * given direction. The caller decides which systems are *present* (reachable +
 * ghosts) and passes them; everything in `present` gets a position — ranked if
 * reachable from the root, parked in the gutter otherwise.
 */
export function layoutSeed(
	graph: CombinedGraph,
	tab: Tab,
	dir: LayoutDirection,
	present: Set<string>
): Positions {
	const adj = buildAdjacency(graph, present);

	// Wildcard / root-less tabs have no anchor: rank from the first present system
	// (stable by `graph.systems` order) so the layout is still connected-looking
	// and deterministic rather than all-gutter.
	// A normal tab seeds from its single root; the wildcard / root-less tab has no
	// anchor, so we rank from the first present system (stable by `graph.systems`
	// order) to keep the layout connected-looking and deterministic. `bfsRanks`
	// still takes a list (it's a multi-source BFS) — we just feed it one seed.
	const roots =
		!tab.isWildcard && tab.root
			? [tab.root]
			: graph.systems.filter((s) => present.has(s.id)).slice(0, 1).map((s) => s.id);

	const rank = bfsRanks(adj, roots);

	// Group reachable nodes by rank, preserving `graph.systems` order for stable
	// sibling indices (insertion order → same input, same layout).
	const byRank = new Map<number, string[]>();
	const gutter: string[] = [];
	for (const s of graph.systems) {
		if (!present.has(s.id)) continue;
		const r = rank.get(s.id);
		if (r === undefined) {
			gutter.push(s.id);
		} else {
			let bucket = byRank.get(r);
			if (!bucket) byRank.set(r, (bucket = []));
			bucket.push(s.id);
		}
	}

	// Order each rank's siblings to reduce edge crossings (barycenter heuristic).
	// Without this, siblings sit in raw `graph.systems` order, so a child can land
	// far from its parent and its edge crosses others. `orderRanks` reseats each
	// rank by the average position of its neighbours in the adjacent rank — pulling
	// edges straighter. Replaces the old insertion-order indices; still pure +
	// deterministic (same input → same order).
	const ranks = [...byRank.entries()].sort((a, b) => a[0] - b[0]);
	orderRanks(ranks, adj);

	const out: Positions = {};
	for (const [r, siblings] of ranks) {
		placeRank(dir, r, siblings, out);
	}
	placeGutter(dir, gutter, out);
	return out;
}

/**
 * Crossing-reduction by the barycenter heuristic, in place over `ranks` (already
 * sorted shallow→deep). Each node's barycenter is the mean ORDER-INDEX of its
 * neighbours in a reference rank; sorting a rank by that value seats children
 * under their parents and uncrosses edges. Our graphs are shallow trees with a
 * few cross-links, so a small fixed number of sweeps converges:
 *
 *   - a DOWN sweep (rank r ordered by rank r-1) straightens a pure tree in one
 *     pass — every node follows its single parent;
 *   - alternating UP sweeps (rank r ordered by rank r+1) settle cross-links and
 *     multi-parent nodes (e.g. a diamond), which a single down sweep can't.
 *
 * Rank 0 (the roots) keeps its given order as the anchor. Ties keep the prior
 * order (stable sort), so the result stays deterministic.
 */
function orderRanks(ranks: [number, string[]][], adj: Map<string, string[]>): void {
	// A node's current index within its own rank — the coordinate the barycenter
	// of the NEXT rank averages over. Rebuilt after each sweep.
	const indexOf = new Map<string, number>();
	const reindex = () => {
		for (const [, siblings] of ranks) siblings.forEach((id, i) => indexOf.set(id, i));
	};
	reindex();

	// Reorder rank `siblings` by the mean index of each node's neighbours that
	// live in the reference rank `refIds`. Nodes with no neighbour in that rank
	// keep their place (barycenter = their own current index).
	const sweep = (siblings: string[], refIds: Set<string>) => {
		const bary = new Map<string, number>();
		for (const id of siblings) {
			const neighbourIdx = adj
				.get(id)!
				.filter((n) => refIds.has(n))
				.map((n) => indexOf.get(n)!);
			bary.set(
				id,
				neighbourIdx.length
					? neighbourIdx.reduce((a, b) => a + b, 0) / neighbourIdx.length
					: indexOf.get(id)!
			);
		}
		// Stable sort by barycenter (ties keep prior order → deterministic).
		siblings
			.map((id, i) => ({ id, b: bary.get(id)!, i }))
			.sort((p, q) => p.b - q.b || p.i - q.i)
			.forEach((e, i) => (siblings[i] = e.id));
	};

	// Two down/up cycles is plenty for shallow chain graphs and keeps it cheap.
	for (let pass = 0; pass < 2; pass++) {
		// DOWN: order rank r by the rank above it (r-1), root rank fixed.
		for (let r = 1; r < ranks.length; r++) {
			sweep(ranks[r][1], new Set(ranks[r - 1][1]));
			reindex();
		}
		// UP: order rank r by the rank below it (r+1), deepest rank fixed.
		for (let r = ranks.length - 2; r >= 1; r--) {
			sweep(ranks[r][1], new Set(ranks[r + 1][1]));
			reindex();
		}
	}
}

/**
 * Park unreached nodes in a visible gutter: a column before the root column (LR),
 * a row above it (TB). They render normally but sit apart so an unreached ghost
 * reads as unreached.
 */
function placeGutter(dir: LayoutDirection, gutter: string[], out: Positions): void {
	gutter.forEach((id, i) => {
		switch (dir) {
			// Park the gutter on the side OPPOSITE the rank flow so unreached nodes
			// sit clear of the ranked chain.
			case 'LR':
				out[id] = { x: -GUTTER_GAP, y: i * DY };
				break;
			case 'RL':
				out[id] = { x: GUTTER_GAP, y: i * DY };
				break;
			case 'TB':
				out[id] = { x: i * DX, y: -GUTTER_GAP };
				break;
			case 'BT':
				out[id] = { x: i * DX, y: GUTTER_GAP };
				break;
		}
	});
}

/**
 * The systems that RENDER for a tab: those reachable from `tab.root` over live
 * connections, plus any ghosts (which park in the gutter). The wildcard tab
 * renders every system. Existence is a pure function of the graph — NEVER of
 * placement. Returned as a Set of ids for layout/reconcile to consume.
 */
export function renderableSystems(
	graph: CombinedGraph,
	tab: Tab,
	ghosts: System[]
): Set<string> {
	const ids = new Set<string>();

	if (tab.isWildcard) {
		for (const s of graph.systems) ids.add(s.id);
	} else {
		const present = new Set(graph.systems.map((s) => s.id));
		const adj = buildAdjacency(graph, present);
		const reach = bfsRanks(adj, [tab.root]); // reachable set = keys of the rank map
		for (const id of reach.keys()) ids.add(id);
	}

	// Ghosts always render (they're what the user added); they park in the gutter
	// unless a live connection already reaches them.
	for (const g of ghosts) ids.add(g.id);
	return ids;
}
