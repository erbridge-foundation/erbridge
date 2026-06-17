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
		statics: []
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

const tab = (roots: string[], extra: Partial<Tab> = {}): Tab => ({
	id: 't',
	label: 't',
	roots,
	...extra
});

const present = (g: CombinedGraph) => new Set(g.systems.map((s) => s.id));

describe('layoutSeed', () => {
	it('is deterministic — same input gives the same output', () => {
		const g = graph();
		const a = layoutSeed(g, tab(['A']), 'LR', present(g));
		const b = layoutSeed(g, tab(['A']), 'LR', present(g));
		expect(a).toEqual(b);
	});

	it('LR places rank along x and siblings along y', () => {
		const g = graph();
		const pos = layoutSeed(g, tab(['A']), 'LR', present(g));
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
		const pos = layoutSeed(g, tab(['A']), 'TB', present(g));
		expect(pos.A.y).toBe(0);
		expect(pos.B.y).toBeGreaterThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('RL mirrors LR — ranks grow leftwards (roots on the right)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab(['A']), 'RL', present(g));
		expect(pos.A.x).toBe(0);
		expect(pos.B.x).toBeLessThan(pos.A.x);
		expect(pos.C.x).toBeLessThan(pos.B.x);
		// Siblings still spread along y.
		expect(pos.C.x).toBe(pos.E.x);
		expect(pos.C.y).not.toBe(pos.E.y);
	});

	it('BT mirrors TB — ranks grow upwards (roots at the bottom)', () => {
		const g = graph();
		const pos = layoutSeed(g, tab(['A']), 'BT', present(g));
		expect(pos.A.y).toBe(0);
		expect(pos.B.y).toBeLessThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('ranks multi-root tabs by the MINIMUM hop across the root set', () => {
		const g = graph();
		// Roots A and D. C is 1 hop from D but 2 from A → min rank 1.
		const pos = layoutSeed(g, tab(['A', 'D']), 'LR', present(g));
		// A and D are both rank 0 → same x.
		expect(pos.A.x).toBe(pos.D.x);
		// C (rank 1 via D) sits one column out from the roots.
		expect(pos.C.x).toBeGreaterThan(pos.A.x);
		// B is rank 1 from A → same column as C.
		expect(pos.B.x).toBe(pos.C.x);
	});

	it('parks disconnected nodes in a visible gutter, apart from the ranks', () => {
		const g = graph();
		const pos = layoutSeed(g, tab(['A']), 'LR', present(g));
		// X has no connection → gutter, to the left of the root column (x < A.x).
		expect(pos.X.x).toBeLessThan(pos.A.x);
	});

	it('orders siblings to follow their parents (barycenter crossing-reduction)', () => {
		// Two roots R1 (top) and R2 (below). R1 connects to child Lo, R2 to child
		// Hi — but Lo/Hi are listed in the WRONG order (Hi first). A naive
		// insertion-order layout would seat Hi above Lo, crossing both edges. The
		// barycenter pass must reseat them so each child sits beside its parent:
		// R1's child above R2's child (no crossing).
		const sys: System[] = ['R1', 'R2', 'Hi', 'Lo'].map((id) => ({
			id,
			name: id,
			class: 'C2',
			statics: []
		}));
		const g: CombinedGraph = {
			systems: sys,
			connections: [conn('r1lo', 'R1', 'Lo'), conn('r2hi', 'R2', 'Hi')],
			tabs: []
		};
		const pos = layoutSeed(g, tab(['R1', 'R2']), 'LR', new Set(sys.map((s) => s.id)));
		// R1 is the first root (y = 0), R2 sits below it (greater y).
		expect(pos.R2.y).toBeGreaterThan(pos.R1.y);
		// Crossing-free ⇒ R1's child (Lo) sits ABOVE R2's child (Hi), mirroring the
		// roots — even though Hi was listed first.
		expect(pos.Lo.y).toBeLessThan(pos.Hi.y);
	});
});

describe('renderableSystems', () => {
	it('returns systems reachable from the tab roots plus ghosts', () => {
		const g = graph();
		// Drop X from the graph; add it back as a ghost so it still renders.
		const noX: CombinedGraph = { ...g, systems: g.systems.filter((s) => s.id !== 'X') };
		const ghosts: System[] = [{ id: 'X', name: 'X', class: 'C2', statics: [] }];
		const ids = renderableSystems(noX, tab(['A']), ghosts);
		expect(ids).toContain('A');
		expect(ids).toContain('D');
		expect(ids).toContain('X'); // ghost
	});

	it('wildcard tab renders every system regardless of reachability', () => {
		const g = graph();
		const ids = renderableSystems(g, tab([], { isWildcard: true }), []);
		// Even X, which is unreachable, renders on the wildcard tab.
		expect(ids).toContain('X');
		expect(ids.size).toBe(g.systems.length);
	});

	it('non-wildcard tab excludes systems unreachable from the roots', () => {
		const g = graph();
		const ids = renderableSystems(g, tab(['A']), []);
		// X is disconnected and not a ghost here → not rendered.
		expect(ids.has('X')).toBe(false);
	});
});
