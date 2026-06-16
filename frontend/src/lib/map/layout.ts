/**
 * Hand-rolled BFS layout seed (Fork 2 — no layout library).
 *
 * `layoutSeed(graph, tab, dir)` is a PURE function: graph → positions. It is the
 * FLOOR the placement overlay sits on (`pos[id] = saved[id] ?? seed[id]`). The
 * graph carries no coordinates; this is where they come from.
 *
 *   rank(node)    = min BFS hop distance from the nearest system in `tab.roots`
 *   sibling(node) = stable index among nodes sharing a rank (insertion order →
 *                   same input, same layout)
 *
 *   LR     : x = rank * DX,    y = sibling * DY
 *   TB     : x = sibling * DX, y = rank * DY
 *   radial : θ = sibling / count(rank) * 2π,  r = rank * DR
 *
 * Systems not reachable from the roots (a ghost the user added that no live
 * connection reaches, or a disconnected fragment) are PARKED in a gutter rank so
 * they stay visible but read as clearly-unreached. The wildcard tab has no roots,
 * so *every* system parks-or-ranks from a synthetic seed: we rank from the whole
 * node set's first element to keep it deterministic and connected-looking.
 */

import type { CombinedGraph, LayoutDirection, Positions, System, Tab, XY } from './types';

// Spacing between ranks / siblings. Tuned for the wireframe's node size; these
// are seed positions only — the user re-drags freely afterwards.
const DX = 260;
const DY = 150;
const DR = 220;
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
 * Multi-source BFS: rank = min hop across the root set. Returns a map of
 * id → rank for every reachable node. Unreached nodes are absent (parked later).
 * Deterministic: roots seed rank 0 in `roots` order; the queue preserves
 * insertion order so equal-rank discovery order is stable.
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

/** Place one rank's siblings deterministically (stable sibling index). */
function placeRank(
	dir: LayoutDirection,
	rank: number,
	siblings: string[],
	out: Positions
): void {
	const count = siblings.length;
	siblings.forEach((id, sibling) => {
		out[id] = positionFor(dir, rank, sibling, count);
	});
}

function positionFor(dir: LayoutDirection, rank: number, sibling: number, count: number): XY {
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
		case 'radial': {
			// Rank 0 (a single root) sits at the origin; deeper ranks fan around it.
			if (rank === 0 && count === 1) return { x: 0, y: 0 };
			const angle = (sibling / Math.max(count, 1)) * 2 * Math.PI;
			const r = rank * DR || DR; // never collapse a non-root ring to the centre
			return { x: Math.round(Math.cos(angle) * r), y: Math.round(Math.sin(angle) * r) };
		}
	}
}

/**
 * Seed positions for the systems of `graph` as viewed through `tab`, in the
 * given direction. The caller decides which systems are *present* (reachable +
 * ghosts) and passes them; everything in `present` gets a position — ranked if
 * reachable from the roots, parked in the gutter otherwise.
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
	const roots =
		tab.roots.length > 0
			? tab.roots
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

	const out: Positions = {};
	for (const [r, siblings] of [...byRank.entries()].sort((a, b) => a[0] - b[0])) {
		placeRank(dir, r, siblings, out);
	}
	placeGutter(dir, gutter, out);
	return out;
}

/**
 * Park unreached nodes in a visible gutter: a column before the root column (LR),
 * a row above it (TB), or a ring left of centre (radial). They render normally
 * but sit apart so an unreached ghost reads as unreached.
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
			case 'radial':
				out[id] = { x: -GUTTER_GAP, y: i * DY };
				break;
		}
	});
}

/**
 * The systems that RENDER for a tab: those reachable from `tab.roots` over live
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
		const reach = bfsRanks(adj, tab.roots); // reachable set = keys of the rank map
		for (const id of reach.keys()) ids.add(id);
	}

	// Ghosts always render (they're what the user added); they park in the gutter
	// unless a live connection already reaches them.
	for (const g of ghosts) ids.add(g.id);
	return ids;
}
