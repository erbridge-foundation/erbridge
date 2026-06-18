import { describe, it, expect } from 'vitest';
import { layoutSeed, renderableSystems } from './layout';
import type { CombinedGraph, Connection, System, Tab } from './types';

/** A tiny linear chain A—B—C—D plus a side branch B—E, and an island X with no
 *  connection (a ghost / disconnected fragment). */
function graph(): CombinedGraph {
	const systems: System[] = ['A', 'B', 'C', 'D', 'E', 'X'].map((id) => ({
		id,
		name: id,
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
		// A is rank 0, B rank 1, C/E rank 2, D rank 3.
		expect(pos.A.x).toBe(0);
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
		expect(pos.A.y).toBe(0);
		expect(pos.B.y).toBeGreaterThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('RL mirrors LR — ranks grow leftwards (root on the right)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'RL', present(g));
		expect(pos.A.x).toBe(0);
		expect(pos.B.x).toBeLessThan(pos.A.x);
		expect(pos.C.x).toBeLessThan(pos.B.x);
		// Siblings still spread along y.
		expect(pos.C.x).toBe(pos.E.x);
		expect(pos.C.y).not.toBe(pos.E.y);
	});

	it('BT mirrors TB — ranks grow upwards (root at the bottom)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'BT', present(g));
		expect(pos.A.y).toBe(0);
		expect(pos.B.y).toBeLessThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('parks disconnected nodes in a visible gutter, apart from the ranks', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'LR', present(g));
		// X has no connection → gutter, to the left of the root column (x < A.x).
		expect(pos.X.x).toBeLessThan(pos.A.x);
	});

	it('orders siblings to follow their parents (barycenter crossing-reduction)', () => {
		// A single root R with two rank-1 children P1 (top) and P2 (below). P1's
		// rank-2 child is Lo, P2's is Hi — but the grandchildren are LISTED in the
		// wrong order (Hi before Lo). A naive insertion-order layout would seat Hi
		// above Lo, crossing the two P→grandchild edges. The barycenter pass must
		// reseat rank 2 so each grandchild sits beside its parent: P1's child above
		// P2's (no crossing).
		const sys: System[] = ['R', 'P1', 'P2', 'Hi', 'Lo'].map((id) => ({
			id,
			name: id,
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
		// P1 is the first child (y = 0), P2 sits below it (greater y).
		expect(pos.P2.y).toBeGreaterThan(pos.P1.y);
		// Crossing-free ⇒ P1's child (Lo) sits ABOVE P2's child (Hi), mirroring the
		// parents — even though Hi was listed first.
		expect(pos.Lo.y).toBeLessThan(pos.Hi.y);
	});
});

describe('renderableSystems', () => {
	it('returns systems reachable from the tab root plus ghosts', () => {
		const g = graph();
		// Drop X from the graph; add it back as a ghost so it still renders.
		const noX: CombinedGraph = { ...g, systems: g.systems.filter((s) => s.id !== 'X') };
		const ghosts: System[] = [
			{ id: 'X', name: 'X', class: 'C2', statics: [], scans: [], structures: [] }
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
