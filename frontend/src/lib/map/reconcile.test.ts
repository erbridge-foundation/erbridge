import { describe, it, expect } from 'vitest';
import {
	combine,
	dropConfirmedGhosts,
	overlayPositions,
	reconcilePlacement
} from './reconcile';
import type { CombinedGraph, LocalState, Positions, System, Tab } from './types';

const sys = (id: string): System => ({ id, name: id, class: 'C2', statics: [] });

/** A—B—C chain rooted at A. */
function server(): CombinedGraph {
	return {
		systems: [sys('A'), sys('B'), sys('C')],
		connections: [
			{ id: 'ab', source: 'A', target: 'B', origin: 'A', wh_type: 'K', mass: 'fresh', eol: false },
			{ id: 'bc', source: 'B', target: 'C', origin: 'B', wh_type: 'K', mass: 'fresh', eol: false }
		],
		tabs: []
	};
}

const emptyLocal: LocalState = { ghostSystems: [], ghostConnections: [] };
const tab: Tab = { id: 't', label: 't', roots: ['A'] };

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

describe('overlayPositions', () => {
	it('saved position beats the layout seed (survive a restart)', () => {
		const saved: Positions = { A: { x: 999, y: 888 } };
		const pos = overlayPositions(server(), tab, emptyLocal, saved, 'LR');
		expect(pos.A).toEqual({ x: 999, y: 888 });
	});

	it('a node with no saved position takes the layout seed', () => {
		const saved: Positions = { A: { x: 999, y: 888 } };
		const pos = overlayPositions(server(), tab, emptyLocal, saved, 'LR');
		// B has no saved pos → its position comes from the seed (rank 1, x > 0).
		expect(pos.B).toBeDefined();
		expect(pos.B.x).toBeGreaterThan(0);
	});

	it('existence is independent of placement — every rendered node gets a position', () => {
		// No saved positions at all; existence still resolves from the graph.
		const pos = overlayPositions(server(), tab, emptyLocal, {}, 'LR');
		expect(Object.keys(pos).sort()).toEqual(['A', 'B', 'C']);
	});

	it('ghosts render (and get a gutter seed) even with no connection', () => {
		const local: LocalState = { ghostSystems: [sys('G')], ghostConnections: [] };
		const pos = overlayPositions(server(), tab, local, {}, 'LR');
		expect(pos.G).toBeDefined();
	});
});

describe('reconcilePlacement', () => {
	it('keeps saved positions for nodes still present', () => {
		const saved: Positions = { A: { x: 1, y: 2 }, B: { x: 3, y: 4 } };
		const next = reconcilePlacement(saved, new Set(['A', 'B', 'C']));
		expect(next.A).toEqual({ x: 1, y: 2 });
		expect(next.B).toEqual({ x: 3, y: 4 });
	});

	it('forgets the saved position of a departed node', () => {
		const saved: Positions = { A: { x: 1, y: 2 }, GONE: { x: 9, y: 9 } };
		const next = reconcilePlacement(saved, new Set(['A']));
		expect(next.GONE).toBeUndefined();
		expect(next.A).toEqual({ x: 1, y: 2 });
	});

	it('leaves new nodes unsaved so they take the seed (new → seed)', () => {
		const saved: Positions = { A: { x: 1, y: 2 } };
		const next = reconcilePlacement(saved, new Set(['A', 'NEW']));
		expect('NEW' in next).toBe(false);
	});
});

describe('ghost → confirmed end to end (no duplicate, no flicker)', () => {
	it('a confirmed ghost renders once from server state after reconcile', () => {
		// Before: G is a ghost, parked. After: server contains G with a connection.
		const before = server();
		const local: LocalState = { ghostSystems: [sys('G')], ghostConnections: [] };

		const after: CombinedGraph = {
			systems: [...before.systems, sys('G')],
			connections: [
				...before.connections,
				{ id: 'cg', source: 'C', target: 'G', origin: 'C', wh_type: 'K', mass: 'fresh', eol: false }
			],
			tabs: []
		};

		const prunedLocal = dropConfirmedGhosts(after, local);
		const union = combine(after, prunedLocal);
		const gCount = union.systems.filter((s) => s.id === 'G').length;
		expect(gCount).toBe(1); // exactly once — no duplicate
		expect(prunedLocal.ghostSystems).toHaveLength(0); // removed from local state
	});
});
