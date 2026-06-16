/**
 * Static combined-graph snapshot for the `/maps/_proto` sandbox. This is the
 * data under test — it has NO node positions (positions come from the layout
 * seed and/or saved placement at render time).
 *
 * The fixture is deliberately wide enough to exercise every render state the
 * encoding rules (Fork 3) must survive:
 *   - every wormhole class C1–C6 and every security tier HS/LS/NS
 *   - every mass state: fresh / half / critical
 *   - an end-of-life (EoL) connection
 *   - a multi-root tab (rank = min hop across the root set)
 *   - the wildcard `*` tab (shows everything, ignores roots — e.g. eve-scout)
 *   - a seeded ghost living in local state (no server connection reaches it yet)
 *
 * A second server snapshot (`updatedGraph`) drives the simulated-SSE reconcile:
 * it adds a system, removes a system, and confirms the ghost as real server
 * state — see reconcile.ts and the sandbox "receive update" affordance.
 */

import type { CombinedGraph, Connection, LocalState, System } from '$lib/map/types';

// ── Systems ────────────────────────────────────────────────────────────────
// Home is the HS anchor; the chain fans out through every class so the canvas
// shows all six class colours plus the three security tiers at once.

const systems: System[] = [
	{ id: 'Jita', name: 'Jita', class: 'HS', statics: [] },
	{ id: 'J100001', name: 'J100001', class: 'C1', statics: [{ code: 'HSa', dest: 'HS' }] },
	{ id: 'J100002', name: 'J100002', class: 'C2', statics: [{ code: 'C3a', dest: 'C3' }] },
	{
		id: 'J100003',
		name: 'J100003',
		class: 'C3',
		statics: [{ code: 'LSa', dest: 'LS' }]
	},
	{ id: 'J100004', name: 'J100004', class: 'C4', statics: [{ code: 'C4a', dest: 'C4' }] },
	{
		id: 'J100005',
		name: 'J100005',
		class: 'C5',
		statics: [
			{ code: 'C5a', dest: 'C5' },
			{ code: 'NSa', dest: 'NS' }
		]
	},
	{ id: 'J100006', name: 'J100006', class: 'C6', statics: [{ code: 'C6a', dest: 'C6' }] },
	// A low-sec exit (reached via J100003's LS static) and a null-sec exit
	// (reached via J100005's NS static) so HS/LS/NS all render.
	{ id: 'Amamake', name: 'Amamake', class: 'LS', statics: [] },
	{ id: 'EC-P8R', name: 'EC-P8R', class: 'NS', statics: [] }
];

// ── Connections ──────────────────────────────────────────────────────────────
// One link per mass state at least once; one EoL link; a second root reachable
// only via J100004 so the multi-root tab has something to min-hop over.

const connections: Connection[] = [
	{
		id: 'c-jita-j1',
		source: 'Jita',
		target: 'J100001',
		origin: 'Jita',
		wh_type: 'K162',
		mass: 'fresh',
		eol: false,
		sig_source: 'ABC-001',
		sig_target: 'XYZ-100'
	},
	{
		id: 'c-j1-j2',
		source: 'J100001',
		target: 'J100002',
		origin: 'J100001',
		wh_type: 'C247',
		mass: 'half',
		eol: false,
		sig_source: 'DEF-002',
		sig_target: 'XYZ-101'
	},
	{
		id: 'c-j2-j3',
		source: 'J100002',
		target: 'J100003',
		origin: 'J100002',
		wh_type: 'D845',
		mass: 'fresh',
		eol: false
	},
	{
		id: 'c-j3-amamake',
		source: 'J100003',
		target: 'Amamake',
		origin: 'J100003',
		wh_type: 'N968',
		mass: 'critical',
		eol: false
	},
	{
		// EoL link — carries the ⚠ glyph + pulse on the edge label.
		id: 'c-j2-j4',
		source: 'J100002',
		target: 'J100004',
		origin: 'J100002',
		wh_type: 'X702',
		mass: 'half',
		eol: true
	},
	{
		id: 'c-j4-j5',
		source: 'J100004',
		target: 'J100005',
		origin: 'J100004',
		wh_type: 'M267',
		mass: 'fresh',
		eol: false
	},
	{
		id: 'c-j5-j6',
		source: 'J100005',
		target: 'J100006',
		origin: 'J100005',
		wh_type: 'H296',
		mass: 'critical',
		eol: false
	},
	{
		id: 'c-j5-ecp8r',
		source: 'J100005',
		target: 'EC-P8R',
		origin: 'J100005',
		wh_type: 'V911',
		mass: 'fresh',
		eol: false
	}
];

/**
 * The home tab is rooted at the home wormhole J100001 (a typical wormhole home,
 * with k-space hanging off it); the multi-root tab is rooted at the two deep
 * systems (J100004 + J100006) to exercise min-hop rank; the wildcard tab shows
 * everything regardless of reachability (origin-filter UX deferred to Track 2).
 */
export const initialGraph: CombinedGraph = {
	systems,
	connections,
	tabs: [
		{ id: 'home', label: 'Home', roots: ['J100001'] },
		{ id: 'deep', label: 'Deep (multi-root)', roots: ['J100004', 'J100006'] },
		{ id: 'all', label: '*', roots: [], isWildcard: true }
	]
};

/**
 * Local state: a single ghost system the user "added" by hand that no server
 * connection reaches yet. It parks in the layout gutter until `updatedGraph`
 * confirms it as real server state (reconcile drops it from local state then).
 */
export const initialLocalState: LocalState = {
	ghostSystems: [{ id: 'J199999', name: 'J199999', class: 'C2', statics: [] }],
	ghostConnections: []
};

/**
 * Second server snapshot for the simulated update. Versus `initialGraph`:
 *   - ADDS  J100007 (a new C4 reached from J100006) — takes the layout seed.
 *   - DROPS EC-P8R and its connection (a departed system) — forgets its placement.
 *   - CONFIRMS the J199999 ghost: it now exists in server state with a real
 *     connection from J100002, so reconcile removes it from local state and it
 *     renders from server state with no flicker/duplicate.
 */
export const updatedGraph: CombinedGraph = {
	systems: [
		...systems.filter((s) => s.id !== 'EC-P8R'),
		// Ghost confirmed as server truth.
		{ id: 'J199999', name: 'J199999', class: 'C2', statics: [{ code: 'C1a', dest: 'C1' }] },
		// Brand-new system — seeds from layout.
		{ id: 'J100007', name: 'J100007', class: 'C4', statics: [] }
	],
	connections: [
		...connections.filter((c) => c.id !== 'c-j5-ecp8r'),
		{
			id: 'c-j2-j199999',
			source: 'J100002',
			target: 'J199999',
			origin: 'J100002',
			wh_type: 'O477',
			mass: 'fresh',
			eol: false
		},
		{
			id: 'c-j6-j7',
			source: 'J100006',
			target: 'J100007',
			origin: 'J100006',
			wh_type: 'U210',
			mass: 'fresh',
			eol: false
		}
	],
	tabs: initialGraph.tabs
};
