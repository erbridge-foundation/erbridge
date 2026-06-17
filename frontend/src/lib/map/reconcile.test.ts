import { describe, it, expect } from 'vitest';
import { combine, dropConfirmedGhosts } from './reconcile';
import type { CombinedGraph, Connection, LocalState, System } from './types';

const sys = (id: string): System => ({ id, name: id, class: 'C2', statics: [] });
const conn = (id: string, a: string, b: string): Connection => ({
	id,
	a: { system: a, sig: null },
	b: { system: b, sig: null },
	mass: 'fresh',
	eol: false
});

/** A—B—C chain rooted at A. */
function server(): CombinedGraph {
	return {
		systems: [sys('A'), sys('B'), sys('C')],
		connections: [conn('ab', 'A', 'B'), conn('bc', 'B', 'C')],
		tabs: []
	};
}

describe('combine (union)', () => {
	it('unions server systems with local ghosts', () => {
		const local: LocalState = { ghostSystems: [sys('G')], ghostConnections: [] };
		const u = combine(server(), local);
		expect(u.systems.map((s) => s.id).sort()).toEqual(['A', 'B', 'C', 'G']);
	});

	it('server wins on id collision — no duplicate when a ghost is also in server', () => {
		// G exists in BOTH local (ghost) and server → rendered once, from server.
		const local: LocalState = { ghostSystems: [{ ...sys('C'), name: 'stale' }], ghostConnections: [] };
		const u = combine(server(), local);
		const cs = u.systems.filter((s) => s.id === 'C');
		expect(cs).toHaveLength(1);
		expect(cs[0].name).toBe('C'); // server copy, not the stale ghost
	});
});

describe('dropConfirmedGhosts', () => {
	it('removes a ghost the server now confirms (ghost → confirmed)', () => {
		const local: LocalState = { ghostSystems: [sys('C'), sys('G')], ghostConnections: [] };
		const next = dropConfirmedGhosts(server(), local);
		// C is in server now → dropped; G is still unconfirmed → kept.
		expect(next.ghostSystems.map((s) => s.id)).toEqual(['G']);
	});
});

describe('ghost → confirmed end to end (no duplicate, no flicker)', () => {
	it('a confirmed ghost renders once from server state after reconcile', () => {
		// Before: G is a ghost, parked. After: server contains G with a connection.
		const before = server();
		const local: LocalState = { ghostSystems: [sys('G')], ghostConnections: [] };

		const after: CombinedGraph = {
			systems: [...before.systems, sys('G')],
			connections: [...before.connections, conn('cg', 'C', 'G')],
			tabs: []
		};

		const prunedLocal = dropConfirmedGhosts(after, local);
		const union = combine(after, prunedLocal);
		const gCount = union.systems.filter((s) => s.id === 'G').length;
		expect(gCount).toBe(1); // exactly once — no duplicate
		expect(prunedLocal.ghostSystems).toHaveLength(0); // removed from local state
	});
});
