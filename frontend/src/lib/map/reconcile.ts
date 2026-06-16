/**
 * Reconcile — the union and the placement overlay (Fork 1).
 *
 *   combined = server-state ∪ localState
 *   render   = reachable(tab.roots, live connections) ∪ local ghosts
 *   pos[id]  = saved[id] ?? layoutSeed(graph, tab, dir)[id]
 *
 * On a graph change (a simulated SSE update) placement reconciles WITHOUT being
 * graph truth:
 *   - dropped id (left the graph)  → forget its saved position
 *   - new id     (entered)         → take the layout seed
 *   - kept id    (still present)   → keep the saved position
 * And a local-only system the server now confirms is dropped from localState, so
 * it renders from server state with no duplicate (the union dedupes by id).
 */

import { layoutSeed, renderableSystems } from './layout';
import type {
	CombinedGraph,
	Connection,
	LayoutDirection,
	LocalState,
	Positions,
	System,
	Tab
} from './types';

/**
 * The union of server state and local state. Server state wins on id collision
 * (a confirmed ghost renders from server truth, not its stale local copy), which
 * also means the transition is seamless — render is always the union.
 */
export function combine(server: CombinedGraph, local: LocalState): CombinedGraph {
	const serverIds = new Set(server.systems.map((s) => s.id));
	const systems: System[] = [
		...server.systems,
		...local.ghostSystems.filter((g) => !serverIds.has(g.id))
	];

	const serverConnIds = new Set(server.connections.map((c) => c.id));
	const connections: Connection[] = [
		...server.connections,
		...local.ghostConnections.filter((c) => !serverConnIds.has(c.id))
	];

	return { systems, connections, tabs: server.tabs };
}

/**
 * Drop from local state any ghost the server now confirms. Returns a NEW
 * LocalState (callers reassign their `$state`). After this, the confirmed system
 * exists only in server state — no promote step, no duplicate.
 */
export function dropConfirmedGhosts(server: CombinedGraph, local: LocalState): LocalState {
	const serverIds = new Set(server.systems.map((s) => s.id));
	const serverConnIds = new Set(server.connections.map((c) => c.id));
	return {
		ghostSystems: local.ghostSystems.filter((g) => !serverIds.has(g.id)),
		ghostConnections: local.ghostConnections.filter((c) => !serverConnIds.has(c.id))
	};
}

/**
 * The placement overlay for a tab: each rendered node's position is its saved
 * position if present, otherwise the layout seed. Computed over the union graph
 * so ghosts get a (gutter) seed too.
 *
 * `saved` is the tab's persisted positions (placement.loadTab). `seed` defaults
 * to a fresh `layoutSeed`, but callers may pass a precomputed one.
 */
export function overlayPositions(
	graph: CombinedGraph,
	tab: Tab,
	local: LocalState,
	saved: Positions,
	dir: LayoutDirection
): Positions {
	// Lay out over the UNION so ghosts (which live in local state, not server
	// `graph.systems`) get a position too — they park in the gutter.
	const union = combine(graph, local);
	const present = renderableSystems(union, tab, local.ghostSystems);
	const seed = layoutSeed(union, tab, dir, present);

	const out: Positions = {};
	for (const id of present) {
		// saved beats seed (survive-restart); a node with no saved pos takes seed.
		out[id] = saved[id] ?? seed[id];
	}
	return out;
}

/**
 * Reconcile saved placement against a NEW render set: keep saved positions for
 * nodes still present, drop saved positions for nodes that left, and leave new
 * nodes unsaved (they'll take the seed via {@link overlayPositions}). Returns the
 * pruned saved map to persist back.
 */
export function reconcilePlacement(saved: Positions, present: Set<string>): Positions {
	const next: Positions = {};
	for (const [id, pos] of Object.entries(saved)) {
		if (present.has(id)) next[id] = pos; // kept → keep
		// departed → forget (simply not copied across)
	}
	// new ids: absent from `saved`, so they stay unsaved → seed at overlay time.
	return next;
}
