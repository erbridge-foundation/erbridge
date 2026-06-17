/**
 * Reconcile — the union of server state and local (ghost) state.
 *
 *   combined = server-state ∪ localState
 *   render   = reachable(tab.roots, live connections) ∪ local ghosts
 *
 * Positions are NOT this module's concern. The map is laid out ONCE on initial
 * load (`layoutSeed`) and thereafter placed incrementally per SSE event
 * (`place-incoming.ts`) — there is no persisted placement overlay to reconcile
 * (Fork 1 reversed: placement is ephemeral, a refresh re-lays-out). This module
 * only resolves EXISTENCE: who is in the graph right now.
 *
 * A local-only system the server later confirms is dropped from localState, so
 * it renders from server state with no duplicate (the union dedupes by id).
 */

import type { CombinedGraph, Connection, LocalState, System } from './types';

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
