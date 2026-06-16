import { describe, it, expect } from 'vitest';
import { layoutSeed, renderableSystems } from './layout';
import type { CombinedGraph, System, Tab } from './types';

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
		connections: [
			{ id: 'ab', source: 'A', target: 'B', origin: 'A', wh_type: 'K', mass: 'fresh', eol: false },
			{ id: 'bc', source: 'B', target: 'C', origin: 'B', wh_type: 'K', mass: 'fresh', eol: false },
			{ id: 'cd', source: 'C', target: 'D', origin: 'C', wh_type: 'K', mass: 'fresh', eol: false },
			{ id: 'be', source: 'B', target: 'E', origin: 'B', wh_type: 'K', mass: 'fresh', eol: false }
		],
		tabs: []
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

	it('radial puts a lone root at the origin and pushes deeper ranks out', () => {
		const g = graph();
		const pos = layoutSeed(g, tab(['A']), 'radial', present(g));
		expect(pos.A).toEqual({ x: 0, y: 0 });
		const rB = Math.hypot(pos.B.x, pos.B.y);
		const rC = Math.hypot(pos.C.x, pos.C.y);
		expect(rC).toBeGreaterThan(rB);
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
