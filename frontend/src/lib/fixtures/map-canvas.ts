/**
 * Static combined-graph snapshot for the `/maps/_proto` sandbox. This is the
 * data under test — it has NO node positions (positions come from the layout
 * seed and/or saved placement at render time).
 *
 * The fixture is deliberately wide enough to exercise every render state the
 * encoding rules (Fork 3) must survive:
 *   - every wormhole class C1–C6, every k-space tier HS/LS/NS, and Pochven (P)
 *   - every mass state: fresh / half / critical
 *   - an end-of-life (EoL) connection
 *   - a second, disconnected chain anchoring its own single-root tab
 *   - the wildcard `*` tab (shows everything, ignores the root — e.g. eve-scout)
 *   - a seeded ghost living in local state (no server connection reaches it yet)
 *
 * An ordered list of SSE-style events (`updateEvents`) drives the simulated
 * live updates: the map is laid out ONCE from `initialGraph` and thereafter each
 * "receive update" replays the next event, placed incrementally (no whole-map
 * re-layout) — see place-incoming.ts and the sandbox "receive update" affordance.
 */

import type {
	CombinedGraph,
	Connection,
	LocalState,
	MapEvent,
	System,
} from "$lib/map/types";

// ── Systems ────────────────────────────────────────────────────────────────
// Home is the HS anchor; the chain fans out through every class so the canvas
// shows all six class colours plus the three security tiers at once.

const systems: System[] = [
	{ id: "Jita", name: "Jita", class: "HS", statics: [] },
	{
		id: "J100001",
		name: "J100001",
		class: "C1",
		// wh_type is the actual wormhole-type code (not displayed yet); dest is the
		// destination class the node surfaces.
		statics: [{ wh_type: "B274", dest: "HS" }],
	},
	{
		id: "J100002",
		name: "J100002",
		class: "C2",
		statics: [{ wh_type: "O883", dest: "C3" }],
	},
	{
		id: "J100003",
		name: "J100003",
		class: "C3",
		statics: [{ wh_type: "N062", dest: "LS" }],
	},
	{
		id: "J100004",
		name: "J100004",
		class: "C4",
		statics: [{ wh_type: "M267", dest: "C4" }],
	},
	{
		id: "J100005",
		name: "J100005",
		class: "C5",
		statics: [
			{ wh_type: "H900", dest: "C5" },
			{ wh_type: "S199", dest: "NS" },
		],
	},
	{
		id: "J100006",
		name: "J100006",
		class: "C6",
		statics: [{ wh_type: "V911", dest: "C6" }],
	},
	{
		id: "J100007",
		name: "J100007",
		class: "C4",
		statics: [{ wh_type: "Y683", dest: "C4" }],
	},
	// A low-sec exit (reached via J100003's LS static) and a null-sec exit
	// (reached via J100005's NS static) so HS/LS/NS all render.
	{ id: "Amamake", name: "Amamake", class: "LS", statics: [] },
	{ id: "EC-P8R", name: "EC-P8R", class: "NS", statics: [] },
	// A Pochven (Triglavian space) exit so the P tier renders too — its own
	// distinct space type, not NS/LS.
	{ id: "Krirald", name: "Krirald", class: "P", statics: [] },
	// A SEPARATE, disconnected small chain (J200001 → J200002), unreachable from
	// the Home chain. It gives the wildcard `*` tab a second cluster to show and
	// anchors its own single-root "Outpost" tab. Deliberately tiny — no loops, no
	// parallel holes — so the second-chain case stays simple.
	{
		id: "J200001",
		name: "J200001",
		class: "C5",
		statics: [{ wh_type: "H296", dest: "NS" }],
	},
	{
		id: "J200002",
		name: "J200002",
		class: "C2",
		statics: [{ wh_type: "D845", dest: "HS" }],
	},
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
		// Fresh + stable — the CALM baseline: fat solid green line, no glyph, no halo.
		id: "c-jita-j1",
		a: { system: "Jita", sig: { id: "ABC-001", type: "K162" } },
		b: { system: "J100001", sig: { id: "XYZ-100", type: "R943" } },
		mass: "fresh",
		ttl_remaining_min: 1400,
		eol: false,
	},
	{
		// Named (C247) on J100001, K162 on J100002. Half mass, < 4 h left → reduced
		// width + long dash + amber clock glyph + a gentle amber WARNING casing.
		id: "c-j1-j2",
		a: { system: "J100001", sig: { id: "DEF-002", type: "C247" } },
		b: { system: "J100002", sig: { id: "XYZ-101", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 180,
		eol: false,
	},
	{
		// No signatures scanned on either side yet — a bare connection. Fresh+stable.
		id: "c-j2-j3",
		a: { system: "J100002", sig: null },
		b: { system: "J100003", sig: null },
		mass: "fresh",
		ttl_remaining_min: 2400,
		eol: false,
	},
	{
		// Critical mass, but plenty of TIME left → thin red solid line + a STATIC red
		// casing (danger), no breathing (motion is reserved for the time axis).
		id: "c-j3-amamake",
		a: { system: "J100003", sig: { id: "PQR-501", type: "N968" } },
		b: { system: "Amamake", sig: { id: "STU-502", type: "K162" } },
		mass: "critical",
		ttl_remaining_min: 600,
		eol: false,
	},
	{
		// < 1 h left, and BOTH ends scanned-but-unidentified (??? / ???) → direction
		// undetermined (neutral mid-edge marker). < 1 h is the CRITICAL visual tier
		// (the actionable "act now" window) → dash-dot + octagon badge + deep red
		// BREATHING halo, identical to an imminent hole.
		id: "c-j2-j4",
		a: { system: "J100002", sig: { id: "GHI-201", type: null } },
		b: { system: "J100004", sig: { id: "JKL-202", type: null } },
		mass: "half",
		ttl_remaining_min: 45,
		eol: false,
	},
	{
		id: "c-j4-j5",
		a: { system: "J100004", sig: { id: "VWX-301", type: "M267" } },
		b: { system: "J100005", sig: { id: "YZA-302", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 900,
		eol: false,
	},
	{
		// The H296 → K162 example: named H296 on J100005, K162 on J100006.
		id: "c-j5-j6",
		a: { system: "J100005", sig: { id: "BCD-401", type: "H296" } },
		b: { system: "J100006", sig: { id: "EFG-402", type: "K162" } },
		mass: "critical",
		ttl_remaining_min: 1200,
		eol: false,
	},
	{
		id: "c-j5-ecp8r",
		a: { system: "J100005", sig: { id: "HIJ-601", type: "V911" } },
		b: { system: "EC-P8R", sig: { id: "KLM-602", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 2000,
		eol: false,
	},
	{
		// A wormhole into Pochven (Triglavian space) off J100004, so the P tier
		// renders. Named C729 on the J-space side, K162 in Pochven.
		id: "c-j4-krirald",
		a: { system: "J100004", sig: { id: "TRG-401", type: "C729" } },
		b: { system: "Krirald", sig: { id: "TRG-402", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1600,
		eol: false,
	},
	// J100006 → J100007: a fresh C4 hung off the C6. Named Y683 on the C6 side,
	// K162 on J100007.
	{
		id: "c-j6-j7",
		a: { system: "J100006", sig: { id: "Y683-701", type: "Y683" } },
		b: { system: "J100007", sig: { id: "K162-702", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1400,
		eol: false,
	},
	// ── Dual connections (two distinct wormholes between the same pair) ──────────
	// A SECOND, independent hole between J100006 ↔ J100007 (alongside c-j6-j7), so
	// the canvas shows two parallel edges bowed apart for exactly one pair — kept
	// away from the busier core so the encoding demo stays readable.
	{
		id: "c-j6-j7-b",
		a: { system: "J100006", sig: { id: "NOP-801", type: "Z142" } },
		b: { system: "J100007", sig: { id: "QRS-802", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 1100,
		eol: false,
	},
	// ── Mass × time combinations (the two states are INDEPENDENT) ────────────────
	// FULL mass (fresh) but IMMINENT closure: a wide-open hole that's about to
	// collapse from AGE — proves mass≠time, and is the key acceptance case: it must
	// draw the eye as strongly as a critical-mass edge (fat green line, dash-dot,
	// octagon badge, deep red BREATHING halo). ALSO one-sided sig: named B274 known
	// at J100003, the far end (J100004) unscanned (???) — direction still known.
	{
		id: "c-j3-j4_freshEol",
		a: { system: "J100003", sig: { id: "MNO-345", type: "B274" } },
		b: { system: "J100004", sig: null },
		mass: "fresh",
		ttl_remaining_min: 5,
		eol: true,
	},
	// The lone connection of the disconnected second chain: J200001 → J200002.
	// Both ends scanned, fresh + stable — a calm baseline edge in its own cluster.
	{
		id: "c-j200001-j200002",
		a: { system: "J200001", sig: { id: "OUT-001", type: "D845" } },
		b: { system: "J200002", sig: { id: "OUT-002", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1300,
		eol: false,
	},
];

/**
 * Each tab is anchored at a SINGLE root (multi-root was dropped — a new root just
 * means a new tab). The home tab roots at the home wormhole J100001 (k-space
 * hanging off it); the "Outpost" tab roots at J200001, the head of the separate
 * disconnected chain; the wildcard tab shows everything regardless of
 * reachability — so it's the one place both chains appear at once (origin-filter
 * UX deferred to Track 2).
 */
export const initialGraph: CombinedGraph = {
	systems,
	connections,
	tabs: [
		{ id: "home", label: "Home", root: "J100001" },
		{ id: "outpost", label: "Outpost", root: "J200001" },
		{ id: "all", label: "*", root: "", isWildcard: true },
	],
};

/**
 * Local state: a single ghost system the user "added" by hand that no server
 * connection reaches yet. It parks in the layout gutter until an `updateEvents`
 * entry confirms it as real server state (the canvas drops it from local state
 * then, so the union dedupes — no duplicate).
 */
export const initialLocalState: LocalState = {
	ghostSystems: [{ id: "J199999", name: "J199999", class: "C2", statics: [] }],
	ghostConnections: [],
};

/**
 * Ordered SSE-style events. The map lays out ONCE from `initialGraph`; each
 * "receive update" replays the next event, placed incrementally. Together they
 * exercise the live paths:
 *   1. ADD a brand-new system (J100008) reached from J100006 — placed one flow-
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
		kind: "add-system",
		system: { id: "J100008", name: "J100008", class: "C4", statics: [] },
		anchor: "J100006",
		connection: {
			id: "c-j6-j8",
			a: { system: "J100006", sig: { id: "U210-901", type: "U210" } },
			b: { system: "J100008", sig: { id: "XYZ-902", type: "K162" } },
			mass: "fresh",
			ttl_remaining_min: 1440,
			eol: false,
		},
	},
	{
		// The ghost J199999 is confirmed by the server, arriving with a real
		// connection from J100002. (Its server copy carries a static the ghost
		// lacked — server truth wins on the union.)
		kind: "add-system",
		system: {
			id: "J199999",
			name: "J199999",
			class: "C2",
			statics: [{ wh_type: "H121", dest: "C1" }],
		},
		anchor: "J100002",
		connection: {
			id: "c-j2-j199999",
			a: { system: "J100002", sig: { id: "O477-801", type: "O477" } },
			b: { system: "J199999", sig: { id: "XYZ-802", type: "K162" } },
			mass: "fresh",
			ttl_remaining_min: 1440,
			eol: false,
		},
	},
	{ kind: "remove-system", id: "EC-P8R" },
];
