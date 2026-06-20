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

// Engine-agnostic contract: both algorithms must satisfy these. Parametrized so a new
// engine added later is held to the same bar. (Each `layoutSeed` call passes the engine
// explicitly via the `algorithm` arg.)
describe.each(['tidy-tree', 'dagre'] as const)('layoutSeed [%s]', (algo) => {
	const seed = (g: CombinedGraph, t: Tab, dir: Parameters<typeof layoutSeed>[2], p: Set<string>, sp = 1) =>
		layoutSeed(g, t, dir, p, sp, algo);

	it('is deterministic — same input gives the same output', () => {
		const g = graph();
		expect(seed(g, tab('A'), 'LR', present(g))).toEqual(seed(g, tab('A'), 'LR', present(g)));
	});

	it('LR places rank along x and siblings along y', () => {
		const g = graph();
		const pos = seed(g, tab('A'), 'LR', present(g));
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
		const pos = seed(g, tab('A'), 'TB', present(g));
		expect(pos.B.y).toBeGreaterThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('RL mirrors LR — ranks grow leftwards (root on the right)', () => {
		const g = graph();
		const pos = seed(g, tab('A'), 'RL', present(g));
		expect(pos.B.x).toBeLessThan(pos.A.x);
		expect(pos.C.x).toBeLessThan(pos.B.x);
		expect(pos.C.x).toBe(pos.E.x);
		expect(pos.C.y).not.toBe(pos.E.y);
	});

	it('BT mirrors TB — ranks grow upwards (root at the bottom)', () => {
		const g = graph();
		const pos = seed(g, tab('A'), 'BT', present(g));
		expect(pos.B.y).toBeLessThan(pos.A.y);
		expect(pos.C.y).toBe(pos.E.y);
		expect(pos.C.x).not.toBe(pos.E.x);
	});

	it('places a disconnected component clear of the primary tree (no overlap)', () => {
		const g = graph();
		const pos = seed(g, tab('A'), 'LR', present(g));
		// X has no connection → its own component, placed apart from the primary chain
		// (tidy-tree stacks it down the cross axis; dagre parks it in the gutter). Either
		// way it shares no (x,y) with any primary node.
		for (const id of ['A', 'B', 'C', 'D', 'E']) {
			expect(pos.X.x === pos[id].x && pos.X.y === pos[id].y).toBe(false);
		}
	});

	it('roots a component at a tab-root system on the wildcard tab', () => {
		// A (client-side) tab roots the chain at the LEAF D. On the wildcard tab — which
		// has no root of its own — the chain must still anchor at D (read from the tab
		// roots, not a system flag), so ranks grow AWAY from D (D—C—B—A) rather than from
		// the degree-hub B.
		const g = graph();
		g.systems = g.systems.filter((s) => s.id !== 'X');
		g.tabs = [tab('D', { id: 'd-tab', label: 'D' })];
		const pos = seed(g, tab('', { isWildcard: true }), 'LR', new Set(g.systems.map((s) => s.id)));
		expect(pos.C.x).toBeGreaterThan(pos.D.x);
		expect(pos.B.x).toBeGreaterThan(pos.C.x);
		expect(pos.A.x).toBeGreaterThan(pos.B.x);
	});

	it('the spacing multiplier widens the cross-axis gap between siblings', () => {
		const g = graph();
		const tight = seed(g, tab('A'), 'LR', present(g), 1);
		const wide = seed(g, tab('A'), 'LR', present(g), 2);
		const sep = (p: typeof tight) => Math.abs(p.C.y - p.E.y);
		expect(sep(wide)).toBeGreaterThan(sep(tight));
	});

	it('the RANK-spacing multiplier widens the rank-axis gap between depth levels', () => {
		// rankSpacing is the 7th arg of layoutSeed (after algorithm); raising it pushes
		// consecutive ranks apart. Probe the A(rank0)→B(rank1) gap on the rank axis (x in LR).
		const g = graph();
		const tight = layoutSeed(g, tab('A'), 'LR', present(g), 1, algo, 1);
		const wide = layoutSeed(g, tab('A'), 'LR', present(g), 1, algo, 2);
		const rankGap = (p: typeof tight) => Math.abs(p.B.x - p.A.x);
		expect(rankGap(wide)).toBeGreaterThan(rankGap(tight));
	});

	it('rank- and cross-spacing are independent — rank spacing leaves the sibling gap alone', () => {
		// Raising ONLY rankSpacing must not change the cross-axis (sibling) separation.
		const g = graph();
		const base = layoutSeed(g, tab('A'), 'LR', present(g), 1, algo, 1);
		const wideRank = layoutSeed(g, tab('A'), 'LR', present(g), 1, algo, 2);
		const siblingGap = (p: typeof base) => Math.abs(p.C.y - p.E.y);
		expect(siblingGap(wideRank)).toBeCloseTo(siblingGap(base), 5);
	});

	it('orders siblings to follow their parents (crossing reduction)', () => {
		// A single root R with two rank-1 children P1 and P2; P1's rank-2 child is Lo,
		// P2's is Hi — LISTED in the wrong order (Hi before Lo). A crossing-free layout
		// must seat each grandchild beside its parent.
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
		const pos = seed(g, tab('R'), 'LR', new Set(sys.map((s) => s.id)));
		expect(pos.Lo.y < pos.Hi.y).toBe(pos.P1.y < pos.P2.y);
	});
});

describe('layoutSeed — dispatch + defaults', () => {
	it('defaults to the dagre engine when algorithm is omitted', () => {
		const g = graph();
		expect(layoutSeed(g, tab('A'), 'LR', present(g))).toEqual(
			layoutSeed(g, tab('A'), 'LR', present(g), 1, 'dagre')
		);
	});

	it('the two engines produce different geometry for the same graph', () => {
		const g = graph();
		const tidy = layoutSeed(g, tab('A'), 'LR', present(g), 1, 'tidy-tree');
		const dagre = layoutSeed(g, tab('A'), 'LR', present(g), 1, 'dagre');
		expect(tidy).not.toEqual(dagre);
	});

	it('defaults the spacing multiplier to 1 (unchanged layout when omitted)', () => {
		const g = graph();
		expect(layoutSeed(g, tab('A'), 'LR', present(g), undefined, 'tidy-tree')).toEqual(
			layoutSeed(g, tab('A'), 'LR', present(g), 1, 'tidy-tree')
		);
	});
});

describe('layoutSeed [tidy-tree] — engine specifics', () => {
	it('node-size-aware rank step: a wide node widens its rank column gap', () => {
		// Two A—B—C chains, identical except the SECOND gives B a long name (a wide node).
		// The wider B pushes the B→C rank gap out (the rank step is sized from node width).
		const mk = (bName: string): CombinedGraph => {
			const systems: System[] = ['A', 'B', 'C'].map((id) => ({
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
		const narrow = layoutSeed(mk('B'), tab('A'), 'LR', new Set(['A', 'B', 'C']), 1, 'tidy-tree');
		const wide = layoutSeed(mk('A-very-long-system-name'), tab('A'), 'LR', new Set(['A', 'B', 'C']), 1, 'tidy-tree');
		expect(wide.C.x - wide.B.x).toBeGreaterThan(narrow.C.x - narrow.B.x);
	});

	it('stacks a disconnected component down the cross axis below the primary', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'LR', present(g), 1, 'tidy-tree');
		// The lone X sits beyond the primary's cross (y) extent — stacked below, not guttered.
		const primaryMaxY = Math.max(pos.A.y, pos.B.y, pos.C.y, pos.D.y, pos.E.y);
		expect(pos.X.y).toBeGreaterThan(primaryMaxY);
	});
});

describe('layoutSeed [dagre] — engine specifics', () => {
	it('parks a disconnected node in the negative-rank gutter', () => {
		const g = graph();
		const pos = layoutSeed(g, tab('A'), 'LR', present(g), 1, 'dagre');
		// The lone X goes to the gutter, to the left of every ranked node (x < all).
		const rankedMinX = Math.min(pos.A.x, pos.B.x, pos.C.x, pos.D.x, pos.E.x);
		expect(pos.X.x).toBeLessThan(rankedMinX);
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
