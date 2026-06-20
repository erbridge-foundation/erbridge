import { describe, it, expect } from 'vitest';
import { layoutSeed, renderableSystems } from './layout';
import type { CombinedGraph, Connection, System, Tab } from './types';

/** A tiny linear chain A—B—C—D plus a side branch B—E, and an island X with no
 *  connection (a ghost / disconnected fragment). */
function graph(): CombinedGraph {
	const systems: System[] = ['A', 'B', 'C', 'D', 'E', 'X'].map((id) => ({
		id,
		name: id,
		eve_system_id: null,
		class: 'C2',
		statics: [],
		scans: [],
		structures: []
	}));
	return {
		systems,
		connections: [conn('ab', 'A', 'B'), conn('bc', 'B', 'C'), conn('cd', 'C', 'D'), conn('be', 'B', 'E')],
		tabs: []
	};
}

/** Minimal connection in the endpoint shape (no sigs needed for layout). */
function conn(id: string, a: string, b: string): Connection {
	return {
		id,
		a: { system: a, sig: null },
		b: { system: b, sig: null },
		mass: 'fresh',
		ttl_remaining_min: 1440,
		eol: false
	};
}

const tab = (root: string, extra: Partial<Tab> = {}): Tab => ({
	id: 't',
	label: 't',
	root,
	...extra
});

const present = (g: CombinedGraph) => new Set(g.systems.map((s) => s.id));

describe('layoutSeed', () => {
	it('is deterministic — same input gives the same output', () => {
		const g = graph();
		const a = layoutSeed(g, tab('A'), 'LR', present(g));
		const b = layoutSeed(g, tab('A'), 'LR', present(g));
		expect(a).toEqual(b);
	});

	it('LR places rank along x and siblings along y', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'LR', present(g));
		// Ranks grow rightwards: A(rank0) < B(1) < C(2) < D(3) in x. (Absolute origin
		// is the engine's business — assert the ordering, not a zero anchor.)
		expect(pos.B.x).toBeGreaterThan(pos.A.x);
		expect(pos.C.x).toBeGreaterThan(pos.B.x);
		expect(pos.D.x).toBeGreaterThan(pos.C.x);
		// C and E share a rank → same x, different y.
		expect(pos.C.x).toBe(pos.E.x);
		expect(pos.C.y).not.toBe(pos.E.y);
	});

	it('TB places rank along y and siblings along x', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'TB', present(g));
		expect(pos.B.y).toBeGreaterThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('RL mirrors LR — ranks grow leftwards (root on the right)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'RL', present(g));
		expect(pos.B.x).toBeLessThan(pos.A.x);
		expect(pos.C.x).toBeLessThan(pos.B.x);
		// Siblings still spread along y.
		expect(pos.C.x).toBe(pos.E.x);
		expect(pos.C.y).not.toBe(pos.E.y);
	});

	it('BT mirrors TB — ranks grow upwards (root at the bottom)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'BT', present(g));
		expect(pos.B.y).toBeLessThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('stacks a disconnected component as a satellite, clear of the primary tree', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'LR', present(g));
		// X has no connection → its own single-node component, stacked DOWN the cross
		// axis (y in LR) below the primary tree, not overlapping it. (The old layout
		// parked it in a negative-rank gutter; the forest now stacks all components on
		// the cross axis, every one oriented the same way.) Its y sits beyond the
		// primary's cross extent, and it shares no (x,y) with any primary node.
		const primaryMaxY = Math.max(pos.A.y, pos.B.y, pos.C.y, pos.D.y, pos.E.y);
		expect(pos.X.y).toBeGreaterThan(primaryMaxY);
		for (const id of ['A', 'B', 'C', 'D', 'E']) {
			expect(pos.X.x === pos[id].x && pos.X.y === pos[id].y).toBe(false);
		}
	});

	it('node-size-aware rank step: a wide node widens its rank column gap', () => {
		// Two A—B—C chains, identical except the SECOND gives B a long name (a wide node).
		// The wider B pushes the B→C rank gap out, so the second chain's rank-1→rank-2 gap
		// is larger than the first's (the rank step is sized from node width, not constant).
		const mk = (bName: string): CombinedGraph => {
			const ids = ['A', 'B', 'C'];
			const systems: System[] = ids.map((id) => ({
				id,
				name: id === 'B' ? bName : id,
				eve_system_id: null,
				class: 'C2',
				statics: [],
				scans: [],
				structures: []
			}));
			return { systems, connections: [conn('ab', 'A', 'B'), conn('bc', 'B', 'C')], tabs: [] };
		};
		const narrow = layoutSeed(mk('B'), tab('A'), 'LR', new Set(['A', 'B', 'C']));
		const wide = layoutSeed(mk('A-very-long-system-name'), tab('A'), 'LR', new Set(['A', 'B', 'C']));
		// The rank-1 (B) → rank-2 (C) centre gap grows when B is wider.
		expect(wide.C.x - wide.B.x).toBeGreaterThan(narrow.C.x - narrow.B.x);
	});

	it('roots a component at its `root`-flagged system on the wildcard tab', () => {
		// Flag a LEAF (D) as root: on the wildcard tab the chain must anchor at D, so
		// ranks grow AWAY from D (D—C—B—A) rather than from the degree-hub B.
		const g = graph();
		g.systems = g.systems.filter((s) => s.id !== 'X');
		g.systems = g.systems.map((s) => (s.id === 'D' ? { ...s, flags: ['root'] as const } : s));
		const pos = layoutSeed(g, tab('', { isWildcard: true }), 'LR', new Set(g.systems.map((s) => s.id)));
		expect(pos.C.x).toBeGreaterThan(pos.D.x);
		expect(pos.B.x).toBeGreaterThan(pos.C.x);
		expect(pos.A.x).toBeGreaterThan(pos.B.x);
	});

	it('the spacing multiplier widens the cross-axis gap between siblings', () => {
		const g = graph();
		// C and E share a rank; their y-separation grows with the spacing multiplier.
		const tight = layoutSeed(g, tab('A'), 'LR', present(g), 1);
		const wide = layoutSeed(g, tab('A'), 'LR', present(g), 2);
		const sep = (p: typeof tight) => Math.abs(p.C.y - p.E.y);
		expect(sep(wide)).toBeGreaterThan(sep(tight));
	});

	it('defaults the spacing multiplier to 1 (unchanged layout when omitted)', () => {
		const g = graph();
		expect(layoutSeed(g, tab('A'), 'LR', present(g))).toEqual(
			layoutSeed(g, tab('A'), 'LR', present(g), 1)
		);
	});

	it('orders siblings to follow their parents (crossing reduction)', () => {
		// A single root R with two rank-1 children P1 and P2; P1's rank-2 child is Lo,
		// P2's is Hi — LISTED in the wrong order (Hi before Lo). A crossing-free layout
		// must seat each grandchild beside its parent: whichever parent is on top, its
		// child is on top too (no P→grandchild edge crossing).
		const sys: System[] = ['R', 'P1', 'P2', 'Hi', 'Lo'].map((id) => ({
			id,
			name: id,
			eve_system_id: null,
			class: 'C2',
			statics: [],
			scans: [],
			structures: []
		}));
		const g: CombinedGraph = {
			systems: sys,
			connections: [
				conn('rp1', 'R', 'P1'),
				conn('rp2', 'R', 'P2'),
				conn('p1lo', 'P1', 'Lo'),
				conn('p2hi', 'P2', 'Hi')
			],
			tabs: []
		};
		const pos = layoutSeed(g, tab('R'), 'LR', new Set(sys.map((s) => s.id)));
		// Crossing-free ⇔ the grandchildren keep their parents' vertical order: Lo
		// (P1's child) is on the same side of Hi (P2's child) as P1 is of P2 — even
		// though Hi was listed first.
		expect(pos.Lo.y < pos.Hi.y).toBe(pos.P1.y < pos.P2.y);
	});
});

describe('renderableSystems', () => {
	it('returns systems reachable from the tab root plus ghosts', () => {
		const g = graph();
		// Drop X from the graph; add it back as a ghost so it still renders.
		const noX: CombinedGraph = { ...g, systems: g.systems.filter((s) => s.id !== 'X') };
		const ghosts: System[] = [
			{ id: 'X', name: 'X', eve_system_id: null, class: 'C2', statics: [], scans: [], structures: [] }
		];
		const ids = renderableSystems(noX, tab('A'), ghosts);
		expect(ids).toContain('A');
		expect(ids).toContain('D');
		expect(ids).toContain('X'); // ghost
	});

	it('wildcard tab renders every system regardless of reachability', () => {
		const g = graph();
		const ids = renderableSystems(g, tab('', { isWildcard: true }), []);
		// Even X, which is unreachable, renders on the wildcard tab.
		expect(ids).toContain('X');
		expect(ids.size).toBe(g.systems.length);
	});

	it('non-wildcard tab excludes systems unreachable from the root', () => {
		const g = graph();
		const ids = renderableSystems(g, tab('A'), []);
		// X is disconnected and not a ghost here → not rendered.
		expect(ids.has('X')).toBe(false);
	});
});
