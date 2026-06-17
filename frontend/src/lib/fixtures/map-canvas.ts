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
 * An ordered list of SSE-style events (`updateEvents`) drives the simulated
 * live updates: the map is laid out ONCE from `initialGraph` and thereafter each
 * "receive update" replays the next event, placed incrementally (no whole-map
 * re-layout) — see place-incoming.ts and the sandbox "receive update" affordance.
 */

import type { CombinedGraph, Connection, LocalState, MapEvent, System } from '$lib/map/types';

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
// Each connection is a pair of endpoints { system, sig }. The wormhole TYPE
// lives on the endpoint signature (a hole is `K162` on one side, a named code on
// the other), so direction is DERIVED — the arrow points toward the K162 end.
// The fixture exercises every signature state:
//   - both ends known      (named + K162)         → direction known
//   - one end known         (named OR K162 only)  → direction still known
//   - both ends unknown     (??? / ???)            → direction undetermined
//   - no signatures at all  (sig: null both ends)  → bare connection, no pills

const connections: Connection[] = [
	{
		// Both ends scanned: Jita holds the K162, J100001 the named side.
		id: 'c-jita-j1',
		a: { system: 'Jita', sig: { id: 'ABC-001', type: 'K162' } },
		b: { system: 'J100001', sig: { id: 'XYZ-100', type: 'R943' } },
		mass: 'fresh',
		eol: false
	},
	{
		// Named (C247) on J100001, K162 on J100002.
		id: 'c-j1-j2',
		a: { system: 'J100001', sig: { id: 'DEF-002', type: 'C247' } },
		b: { system: 'J100002', sig: { id: 'XYZ-101', type: 'K162' } },
		mass: 'half',
		eol: false
	},
	{
		// No signatures scanned on either side yet — a bare connection.
		id: 'c-j2-j3',
		a: { system: 'J100002', sig: null },
		b: { system: 'J100003', sig: null },
		mass: 'fresh',
		eol: false
	},
	{
		id: 'c-j3-amamake',
		a: { system: 'J100003', sig: { id: 'PQR-501', type: 'N968' } },
		b: { system: 'Amamake', sig: { id: 'STU-502', type: 'K162' } },
		mass: 'critical',
		eol: false
	},
	{
		// EoL link, and BOTH ends scanned-but-unidentified (??? / ???) → direction
		// undetermined: the neutral mid-edge marker renders instead of an arrow.
		id: 'c-j2-j4',
		a: { system: 'J100002', sig: { id: 'GHI-201', type: null } },
		b: { system: 'J100004', sig: { id: 'JKL-202', type: null } },
		mass: 'half',
		eol: true
	},
	{
		id: 'c-j4-j5',
		a: { system: 'J100004', sig: { id: 'VWX-301', type: 'M267' } },
		b: { system: 'J100005', sig: { id: 'YZA-302', type: 'K162' } },
		mass: 'fresh',
		eol: false
	},
	{
		// The H296 → K162 example: named H296 on J100005, K162 on J100006.
		id: 'c-j5-j6',
		a: { system: 'J100005', sig: { id: 'BCD-401', type: 'H296' } },
		b: { system: 'J100006', sig: { id: 'EFG-402', type: 'K162' } },
		mass: 'critical',
		eol: false
	},
	{
		id: 'c-j5-ecp8r',
		a: { system: 'J100005', sig: { id: 'HIJ-601', type: 'V911' } },
		b: { system: 'EC-P8R', sig: { id: 'KLM-602', type: 'K162' } },
		mass: 'fresh',
		eol: false
	},
	// ── Dual connections (two distinct wormholes between the same pair) ──────────
	// J100002 ↔ J100003 already has c-j2-j3; add a second, independent hole so the
	// canvas shows two parallel edges between one pair.
	{
		id: 'c-j2-j3-b',
		a: { system: 'J100002', sig: { id: 'NOP-701', type: 'Z142' } },
		b: { system: 'J100003', sig: { id: 'QRS-702', type: 'K162' } },
		mass: 'half',
		eol: false
	},
	// ── Mass × time combinations (the two states are INDEPENDENT) ────────────────
	// FULL mass (fresh) but <10% time left (EoL): a wide-open hole that's about to
	// collapse from age — proves mass≠time. ALSO one-sided sig: named B274 known at
	// the J100003 end, the far (J100004) end not yet scanned (???) — direction is
	// still known (named side fixes it).
	{
		id: 'c-j3-j4_freshEol',
		a: { system: 'J100003', sig: { id: 'MNO-345', type: 'B274' } },
		b: { system: 'J100004', sig: null },
		mass: 'fresh',
		eol: true
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
 * connection reaches yet. It parks in the layout gutter until an `updateEvents`
 * entry confirms it as real server state (the canvas drops it from local state
 * then, so the union dedupes — no duplicate).
 */
export const initialLocalState: LocalState = {
	ghostSystems: [{ id: 'J199999', name: 'J199999', class: 'C2', statics: [] }],
	ghostConnections: []
};

/**
 * Ordered SSE-style events. The map lays out ONCE from `initialGraph`; each
 * "receive update" replays the next event, placed incrementally. Together they
 * exercise the live paths:
 *   1. ADD a brand-new system (J100007) reached from J100006 — placed one flow-
 *      step out from its anchor, then collisions ripple across the graph.
 *   2. CONFIRM the J199999 ghost: it arrives as a real server system with a
 *      connection from J100002. The canvas drops it from local state (so the
 *      union dedupes — no duplicate) and re-anchors it to J100002.
 *   3. REMOVE EC-P8R (a departed system); its edge drops with it.
 *
 * Replaying past the end is a no-op (the sandbox button just stops doing
 * anything once the script is exhausted).
 */
export const updateEvents: MapEvent[] = [
	{
		kind: 'add-system',
		system: { id: 'J100007', name: 'J100007', class: 'C4', statics: [] },
		anchor: 'J100006',
		connection: {
			id: 'c-j6-j7',
			a: { system: 'J100006', sig: { id: 'U210-901', type: 'U210' } },
			b: { system: 'J100007', sig: { id: 'XYZ-902', type: 'K162' } },
			mass: 'fresh',
			eol: false
		}
	},
	{
		// The ghost J199999 is confirmed by the server, arriving with a real
		// connection from J100002. (Its server copy carries a static the ghost
		// lacked — server truth wins on the union.)
		kind: 'add-system',
		system: { id: 'J199999', name: 'J199999', class: 'C2', statics: [{ code: 'C1a', dest: 'C1' }] },
		anchor: 'J100002',
		connection: {
			id: 'c-j2-j199999',
			a: { system: 'J100002', sig: { id: 'O477-801', type: 'O477' } },
			b: { system: 'J199999', sig: { id: 'XYZ-802', type: 'K162' } },
			mass: 'fresh',
			eol: false
		}
	},
	{ kind: 'remove-system', id: 'EC-P8R' }
];
