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
 *   - a wide, deeply-branched "DEEP" chain (modelled on a Wanderer screenshot)
 *     anchoring its own single-root tab — exercises a big multi-rank fan-out
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
	ScanResult,
	Structure,
	System,
	TrackingMeta,
} from "$lib/map/types";

// ── Tracking metadata helper ─────────────────────────────────────────────────
// Every scan/structure carries who/when provenance (ISO-8601 UTC). The fixture
// uses a couple of fixed pilot char ids and computes ISO timestamps "N minutes
// ago" so the sidebar's relative-"Updated" column shows realistic, stable-ish
// values. Kept tiny so each record below stays a one-liner.
const PILOT_A = 91000001;
const PILOT_B = 91000002;
// Real scanner char ids carried by the ingested DEEP signatures (from the live
// Wanderer JSON's `character_eve_id`). The model stores char ids (numeric); name
// resolution for display is a render-time concern, so we keep the ids verbatim.
const WANDERER_PILOTS = {
	a: 1377935050,
	b: 96302482,
	c: 95969364,
	d: 860150027,
	e: 2115168357,
} as const;
const minsAgo = (n: number): string =>
	new Date(Date.now() - n * 60_000).toISOString();
/** created `c` mins ago by `cb`, last updated `u` mins ago by `ub`. */
const track = (c: number, cb: number, u: number, ub: number): TrackingMeta => ({
	created_at: minsAgo(c),
	created_by: cb,
	updated_at: minsAgo(u),
	updated_by: ub,
});

// ── Systems ────────────────────────────────────────────────────────────────
// Home is the HS anchor; the chain fans out through every class so the canvas
// shows all six class colours plus the three security tiers at once.
//
// Two systems carry real scanned content so the sidebar Signatures/Structures
// sections have varied data to bind to:
//   - J100003 (C3): a spread of scan groups/types — wormhole / data / gas
//     (partial) / ore anomaly / an unknown sig — exercising the scanIs* states.
//   - J100005 (C5): two structures from DIFFERENT sources — a probe-scanned
//     Fortizar (links a ScanResult by sig_id) and an overview-paste structure
//     WITH a reinforcement timer (name + owner only).
//   - J100006 (C6): the d-scan case — two Astrahus from a d-scan paste (same
//     hull, distinct names), no sig_id/owner.
// Every other system has empty scans/structures.

// J100003's signatures: the full spread of scan states.
const j100003Scans: ScanResult[] = [
	{
		// Resolved wormhole — its wh_type matches this system's endpoint sig on
		// c-j3-amamake (N968), the "scan ↔ connection reference" link.
		sig_id: "PQR-501",
		group: "Cosmic Signature",
		site_type: "Wormhole",
		name: "Unstable Wormhole",
		wh_type: "N968",
		...track(120, PILOT_A, 8, PILOT_A),
	},
	{
		sig_id: "DAT-110",
		group: "Cosmic Signature",
		site_type: "Data Site",
		name: "Unsecured Perimeter Transponder Farm",
		wh_type: null,
		...track(95, PILOT_A, 30, PILOT_B),
	},
	{
		sig_id: "REL-115",
		group: "Cosmic Signature",
		site_type: "Relic Site",
		name: "Ruined Rogue Drone Monument Site",
		wh_type: null,
		...track(80, PILOT_A, 18, PILOT_A),
	},
	{
		// Partial: classified as gas but not yet named (site_type set, name null) —
		// exercises scanIsPartial + map-search-by-"gas" later. (Once resolved it
		// would read "Vital Core Reservoir".)
		sig_id: "GAS-220",
		group: "Cosmic Signature",
		site_type: "Gas Site",
		name: null,
		wh_type: null,
		...track(60, PILOT_B, 60, PILOT_B),
	},
	{
		sig_id: "ORE-330",
		group: "Cosmic Anomaly",
		site_type: "Ore Site",
		name: "Average Frontier Deposit",
		wh_type: null,
		...track(45, PILOT_A, 5, PILOT_A),
	},
	{
		// Unknown: freshly bookmarked, nothing identified yet (site_type null).
		sig_id: "UNK-440",
		group: "Cosmic Signature",
		site_type: null,
		name: null,
		wh_type: null,
		...track(2, PILOT_B, 2, PILOT_B),
	},
];

// J100005's structures: scanner + overview (with timer) sources.
const j100005Structures: Structure[] = [
	{
		// Probe-scanned: also appears as a 'Structure'-group scan (linked by sig_id).
		id: "st-j5-fort",
		name: "J100005 - Home Fort",
		type_id: 35833,
		hull: "Fortizar",
		owner: "Brave Collective",
		sig_id: "STR-550",
		timer: null,
		source: "scanner",
		...track(200, PILOT_A, 50, PILOT_A),
	},
	{
		// Overview-selected paste: name + owner only, but carries a reinforcement
		// timer — the only source that ever does.
		id: "st-j5-astra",
		name: "J100005 - Forward Op",
		type_id: null,
		hull: null,
		owner: "Hostile Corp",
		sig_id: null,
		timer: { state: "reinforced", ends_at: minsAgo(-1440) }, // ~24h out
		source: "overview",
		...track(20, PILOT_B, 12, PILOT_B),
	},
];
// The scanner-sourced structure also shows up as a probe scan in the system.
const j100005Scans: ScanResult[] = [
	{
		sig_id: "STR-550",
		group: "Structure",
		site_type: "Citadel",
		name: "Fortizar",
		wh_type: null,
		...track(200, PILOT_A, 50, PILOT_A),
	},
];

// J100006's structures: the d-scan case — two Astrahus (same hull, distinct
// names), no sig_id/owner (d-scan carries neither).
const j100006Structures: Structure[] = [
	{
		id: "st-j6-boags",
		name: "J100006 - Boags Brewery",
		type_id: 35832,
		hull: "Astrahus",
		owner: null,
		sig_id: null,
		timer: null,
		source: "dscan",
		...track(40, PILOT_A, 15, PILOT_A),
	},
	{
		id: "st-j6-belly",
		name: "J100006 - Belly Button Zest",
		type_id: 35832,
		hull: "Astrahus",
		owner: null,
		sig_id: null,
		timer: null,
		source: "dscan",
		...track(40, PILOT_A, 15, PILOT_A),
	},
];

// ── DEEP-chain scans: ingested from the live Wanderer JSON ────────────────────
// These lists are the REAL signatures the JSON reports per `solar_system_id`
// (see the eve_system_id backfill on each DEEP system below). The ingest maps each
// JSON row → ScanResult: `eve_id`→sig_id, `kind`→group, `group`→site_type,
// `type`→wh_type, `name`→name (null when the JSON name is "Unknown"/null),
// `character_eve_id`→created_by/updated_by. Farm dumps (30+ Core Garrison rows)
// are trimmed to a representative handful — the WORMHOLE sigs are kept in full
// because their real `wh_type` codes (K162 vs named H296/C140/E175/…) are what
// `k162End()` reads to orient the direction arrows.
const P = WANDERER_PILOTS;
/** Compact ScanResult builder for ingested rows. `wh` defaults null (non-wormhole). */
const scan = (
	sig_id: string,
	site_type: string | null,
	name: string | null,
	by: number,
	mins: number,
	wh: string | null = null,
): ScanResult => ({
	sig_id,
	group:
		site_type === null && name === null
			? "Cosmic Signature"
			: site_type === "Ore Site"
				? "Cosmic Anomaly"
				: "Cosmic Signature",
	site_type,
	name,
	wh_type: wh,
	...track(mins + 60, by, mins, by),
});

// Root 31002274 (J172840). Five data/relic cosmic sites plus the wormhole sigs that
// ARE this system's branch endpoints — their sig_ids match the root-side endpoint
// sigs on the c-deep-root-* connections (the scan ↔ connection reference link).
const j172840Scans: ScanResult[] = [
	scan("DDS-065", "Data Site", "Unsecured Frontier Enclave Relay", P.d, 51),
	scan("GLV-240", "Data Site", "Unsecured Frontier Enclave Relay", P.b, 51),
	scan("HHP-171", "Data Site", "Unsecured Frontier Server Bank", P.e, 51),
	scan("MLY-760", "Data Site", "Unsecured Frontier Enclave Relay", P.d, 51),
	scan("RFS-836", "Data Site", "Unsecured Frontier Server Bank", P.e, 51),
	scan("YBB-406", "Relic Site", "Forgotten Core Data Field", P.e, 51),
	// → J140717 (RMZ branch); ageing fast in the screenshot (~6 min) — TTL on the edge.
	scan("RMZ-780", "Wormhole", "Unstable Wormhole", P.a, 4, "K162"),
	// → J100858 (SWE branch).
	scan("SWE-200", "Wormhole", "Unstable Wormhole", P.a, 51, "K162"),
	// → Sarline (TAR branch); C140 names this side.
	scan("TAR-387", "Wormhole", "Unstable Wormhole", P.a, 51, "C140"),
	// → J120922 (WAT branch); H296 names this side, K162 on J120922.
	scan("WAT-512", "Wormhole", "Unstable Wormhole", P.b, 51, "H296"),
];

// 31002322 (J120922, WAT branch head). Real data sites + its three wormholes.
const j120922Scans: ScanResult[] = [
	scan("WHV-000", "Data Site", "Unsecured Frontier Enclave Relay", P.b, 60),
	scan("DTT-755", "Data Site", null, P.b, 60),
	scan("TSN-764", "Data Site", "Unsecured Frontier Enclave Relay", P.b, 60),
	scan("XTZ-325", "Data Site", "Unsecured Frontier Server Bank", P.b, 60),
	scan("VHS-446", "Ore Site", "Average Frontier Deposit", P.a, 90),
	scan("OGJ-470", "Wormhole", "Unstable Wormhole", P.b, 60, "E175"),
	scan("SOH-872", "Wormhole", "Unstable Wormhole", P.b, 60, "K162"),
	scan("ZVD-547", "Wormhole", "Unstable Wormhole", P.b, 60, "X877"),
	scan("ROG-553", "Wormhole", "Unstable Wormhole", P.b, 60, null),
	scan("DWM-464", "Wormhole", null, P.c, 30, "M267"),
];

// 31001677 (J113551, DWM branch). Data/gas sites + its wormholes.
const j113551Scans: ScanResult[] = [
	scan("NHL-392", "Data Site", "Unsecured Frontier Trinary Hub", P.c, 40),
	scan("ZZT-325", "Data Site", "Unsecured Frontier Digital Nexus", P.c, 40),
	scan("WED-040", "Data Site", "Unsecured Frontier Digital Nexus", P.c, 40),
	scan("SHO-320", "Data Site", "Unsecured Frontier Trinary Hub", P.c, 40),
	scan("XJD-996", "Gas Site", null, P.c, 40),
	scan("NCU-781", "Gas Site", null, P.c, 40),
	scan("RVJ-220", "Gas Site", null, P.c, 40),
	scan("RTU-152", "Wormhole", "Unstable Wormhole", P.c, 40, "K162"),
	scan("KLR-720", "Wormhole", "Unstable Wormhole", P.c, 40, "X877"),
	scan("TJE-744", "Wormhole", "Unstable Wormhole", P.c, 40, "M267"),
	scan("XUP-973", "Wormhole", null, P.c, 40, null),
];

// 31001130 (J105409). Relic/data sites + its wormhole.
const j105409Scans: ScanResult[] = [
	scan("GAD-600", "Relic Site", null, P.c, 45),
	scan("QZT-536", "Relic Site", null, P.c, 45),
	scan("MMP-520", "Data Site", "Unsecured Frontier Receiver", P.c, 45),
	scan("WCF-732", null, null, P.c, 45),
	// Two wormholes: MMY-968 is the inbound (from J113551), HGO-317 the outbound static
	// (D845 → Charmerout). Both correspond to c-deep-* edge endpoints below.
	scan("MMY-968", "Wormhole", "Unstable Wormhole", P.c, 45, "K162"),
	scan("HGO-317", "Wormhole", "Unstable Wormhole", P.c, 45, "D845"),
];

// 31002024 (J100858, SWE branch). Ore/combat farm (trimmed) + its two wormholes.
const j100858Scans: ScanResult[] = [
	scan("JEK-168", "Ore Site", "Common Perimeter Deposit", P.a, 80),
	scan("CLL-452", "Combat Site", "Core Stronghold", P.a, 80),
	scan("QWQ-060", "Wormhole", "Unstable Wormhole", P.a, 50, "H296"),
	scan("NBK-200", "Wormhole", null, P.a, 50, "H296"),
];

// 31000416 (J150606). A rich mix — data/relic/combat + several wormholes (good
// direction-arrow coverage: O477, K162×2, B274, Q003).
const j150606Scans: ScanResult[] = [
	scan("BZH-958", "Data Site", null, P.a, 30),
	scan("OVR-924", "Data Site", null, P.a, 30),
	scan("AFJ-660", "Relic Site", null, P.a, 30),
	scan("ILN-810", "Combat Site", "The Ruins of Enclave Cohort 27", P.a, 30),
	scan("TIK-783", "Wormhole", "Unstable Wormhole", P.a, 30, "O477"),
	scan("DLD-385", "Wormhole", "Unstable Wormhole", P.a, 30, "K162"),
	scan("QSY-488", "Wormhole", "Unstable Wormhole", P.a, 30, "K162"),
	scan("EYD-752", "Wormhole", "Unstable Wormhole", P.a, 30, "B274"),
	scan("BOP-899", "Wormhole", "Unstable Wormhole", P.a, 30, "Q003"),
];

// Lighter ingested sets for the remaining DEEP "farm" systems so their sidebars +
// node sig cues aren't empty. 31002579 (J010951) and 31002517 (J211517) are huge
// Core-* farms in the JSON — trimmed to a handful each.
const j010951Scans: ScanResult[] = [
	scan("EUC-080", "Data Site", null, P.a, 70),
	scan("QRB-786", "Gas Site", null, P.a, 70),
	scan("VRX-242", "Ore Site", "Shattered Ice Field", P.a, 70),
	// Five wormholes: one inbound (ZZX-559, from J100858) and four outbound branches
	// (RJP-460/VOP-803/XEC-225/AWF-387) — each is a c-deep-qwg-* edge endpoint below.
	scan("ZZX-559", "Wormhole", "Unstable Wormhole", P.a, 55, "K162"),
	scan("RJP-460", "Wormhole", "Unstable Wormhole", P.a, 55, "H296"),
	scan("VOP-803", "Wormhole", "Unstable Wormhole", P.a, 55, "H296"),
	scan("XEC-225", "Wormhole", "Unstable Wormhole", P.a, 55, "H296"),
	scan("AWF-387", "Wormhole", "Unstable Wormhole", P.a, 55, "V911"),
];
const j211517Scans: ScanResult[] = [
	scan("UGE-824", "Relic Site", null, P.a, 65),
	scan("KVT-656", "Ore Site", "Rarified Core Deposit", P.a, 65),
	scan("ODC-412", "Wormhole", "Unstable Wormhole", P.a, 55, "K162"),
	scan("RWN-930", "Wormhole", "Unstable Wormhole", P.a, 55, "R474"),
];

// Wormhole-only scan sets for the remaining DEEP systems that had EMPTY scans but
// sit on a c-deep-* edge. Every endpoint sig on a DEEP connection must correspond to
// a Wormhole-type scan in that endpoint's system (the scan ↔ connection link — so an
// edge pill always matches a sidebar row). Each sig id here is reused verbatim as the
// endpoint sig id on the matching connection below. Types are kept real-plausible
// (named code on the upstream side, K162 on the downstream) so `k162End()` resolves.
const j140717Scans: ScanResult[] = [
	scan("JVK-855", "Wormhole", "Unstable Wormhole", P.a, 4, "X877"),
];
const sarlineScans: ScanResult[] = [
	scan("XVK-111", "Wormhole", "Unstable Wormhole", P.a, 51, "K162"),
	scan("VYR-775", "Wormhole", "Unstable Wormhole", P.a, 60, "N062"),
];
const j153054Scans: ScanResult[] = [
	scan("RWR-551", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
	scan("GAS-700", "Gas Site", "Vital Core Reservoir", P.a, 70),
];
const j162332Scans: ScanResult[] = [
	scan("LBI-075", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
	scan("RKP-689", "Wormhole", "Unstable Wormhole", P.a, 60, "D845"),
];
const j150921Scans: ScanResult[] = [
	scan("RXD-181", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
	scan("WVM-085", "Wormhole", "Unstable Wormhole", P.a, 60, "O883"),
	scan("LCZ-027", "Wormhole", "Unstable Wormhole", P.a, 60, "D845"),
];
const charmeroutScans: ScanResult[] = [
	scan("JDX-488", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const j134301Scans: ScanResult[] = [
	scan("GUS-961", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const hatakaniScans: ScanResult[] = [
	scan("OCL-374", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const hurjafrenScans: ScanResult[] = [
	scan("IGS-363", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
	scan("SWE-233", "Wormhole", "Unstable Wormhole", P.a, 60, "Z647"),
];
const j143517Scans: ScanResult[] = [
	scan("RTI-914", "Wormhole", "Unstable Wormhole", P.a, 51, "K162"),
];
const j145512Scans: ScanResult[] = [
	scan("GBZ-653", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const j152912Scans: ScanResult[] = [
	scan("WLF-919", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const rzuolScans: ScanResult[] = [
	scan("RVS-284", "Wormhole", "Unstable Wormhole", P.a, 51, "K162"),
];
const j013070Scans: ScanResult[] = [
	scan("CIT-497", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
	scan("SRB-511", "Wormhole", "Unstable Wormhole", P.a, 60, "V911"),
	scan("ORX-926", "Wormhole", "Unstable Wormhole", P.a, 60, "V911"),
	scan("ENV-256", "Wormhole", "Unstable Wormhole", P.a, 60, "N062"),
];
const j152722Scans: ScanResult[] = [
	scan("HFZ-857", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];
const j110413Scans: ScanResult[] = [
	scan("KFP-798", "Wormhole", "Unstable Wormhole", P.a, 60, "K162"),
];

const systems: System[] = [
	{ id: "Jita", name: "Jita", eve_system_id: 30000142, class: "HS", statics: [], scans: [], structures: [] },
	{
		id: "J100001",
		name: "J100001",
		eve_system_id: 31000001,
		class: "C1",
		// wh_type is the actual wormhole-type code (not displayed yet); dest is the
		// destination class the node surfaces.
		statics: [{ wh_type: "B274", dest: "HS" }],
		scans: [],
		structures: [],
	},
	{
		id: "J100002",
		name: "J100002",
		eve_system_id: 31000002,
		class: "C2",
		statics: [{ wh_type: "O883", dest: "C3" }],
		scans: [],
		structures: [],
	},
	{
		id: "J100003",
		name: "J100003",
		eve_system_id: 31000003,
		class: "C3",
		statics: [{ wh_type: "N062", dest: "LS" }],
		scans: j100003Scans,
		structures: [],
	},
	{
		id: "J100004",
		name: "J100004",
		eve_system_id: 31000004,
		class: "C4",
		statics: [{ wh_type: "M267", dest: "C4" }],
		scans: [],
		structures: [],
	},
	{
		id: "J100005",
		name: "J100005",
		eve_system_id: 31000005,
		class: "C5",
		statics: [
			{ wh_type: "H900", dest: "C5" },
			{ wh_type: "S199", dest: "NS" },
		],
		scans: j100005Scans,
		structures: j100005Structures,
	},
	{
		id: "J100006",
		name: "J100006",
		eve_system_id: 31000006,
		class: "C6",
		statics: [{ wh_type: "V911", dest: "C6" }],
		scans: [],
		structures: j100006Structures,
	},
	{
		id: "J100007",
		name: "J100007",
		eve_system_id: 31000007,
		class: "C4",
		statics: [{ wh_type: "Y683", dest: "C4" }],
		scans: [],
		structures: [],
	},
	// A low-sec exit (reached via J100003's LS static) and a null-sec exit
	// (reached via J100005's NS static) so HS/LS/NS all render.
	{ id: "Amamake", name: "Amamake", eve_system_id: 30002537, class: "LS", statics: [], scans: [], structures: [] },
	{ id: "EC-P8R", name: "EC-P8R", eve_system_id: 30001984, class: "NS", statics: [], scans: [], structures: [] },
	// A Pochven (Triglavian space) exit so the P tier renders too — its own
	// distinct space type, not NS/LS.
	{ id: "Krirald", name: "Krirald", eve_system_id: 30002079, class: "P", statics: [], scans: [], structures: [] },
	// Two more systems hung down-chain off J100007 to demo CRITICAL MASS combined
	// with the two pulsing TTL tiers (the dash texture is gone — only the halo carries
	// TTL now, so a thin red crit-mass line needs a warning/critical pulse beside it):
	//   J100009 — crit-mass + WARNING ttl (amber pulse on a thin red line)
	//   J100010 — crit-mass + CRITICAL ttl (red pulse on a thin red line)
	{
		id: "J100009",
		name: "J100009",
		eve_system_id: 31000009,
		class: "C5",
		statics: [{ wh_type: "N062", dest: "LS" }],
		scans: [],
		structures: [],
	},
	{
		id: "J100010",
		name: "J100010",
		eve_system_id: 31000010,
		class: "C3",
		statics: [{ wh_type: "O883", dest: "C3" }],
		scans: [],
		structures: [],
	},
	// A SEPARATE, disconnected small chain (J200001 → J200002), unreachable from
	// the Home chain. It gives the wildcard `*` tab a second cluster to show and
	// anchors its own single-root "Outpost" tab. Deliberately tiny — no loops, no
	// parallel holes — so the second-chain case stays simple.
	{
		id: "J200001",
		name: "J200001",
		eve_system_id: 31000201,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "NS" }],
		scans: [],
		structures: [],
	},
	{
		id: "J200002",
		name: "J200002",
		eve_system_id: 31000202,
		class: "C2",
		statics: [{ wh_type: "D845", dest: "HS" }],
		scans: [],
		structures: [],
	},
	// ── DEEP chain ───────────────────────────────────────────────────────────
	// A wide, deeply-branched chain modelled on a Wanderer map screenshot. It
	// anchors its own single-root "DEEP" tab at J172840 and fans out three+ ranks
	// deep so the LR layout shows the full tree (the screenshot is left-to-right).
	// Classes / statics / a few names mirror the screenshot; pilot counts, jump-to-
	// Jita labels and eve-scout merge badges aren't in our model, so they're dropped.
	{
		id: "J172840",
		name: "J172840",
		eve_system_id: 31002274,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "C5" }],
		scans: j172840Scans,
		structures: [],
	},
	{
		id: "J120922",
		name: "J120922",
		eve_system_id: 31002322,
		class: "C5",
		statics: [{ wh_type: "M267", dest: "C4" }],
		scans: j120922Scans,
		structures: [],
	},
	{
		id: "J153054",
		name: "J153054",
		eve_system_id: 31000873,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "NS" }],
		scans: j153054Scans,
		structures: [],
	},
	{
		id: "J113551",
		name: "J113551",
		eve_system_id: 31001677,
		class: "C4",
		statics: [
			{ wh_type: "X877", dest: "C2" },
			{ wh_type: "M267", dest: "C3" },
		],
		scans: j113551Scans,
		structures: [],
	},
	{
		id: "J105409",
		name: "J105409",
		eve_system_id: 31001130,
		class: "C3",
		statics: [{ wh_type: "D845", dest: "HS" }],
		scans: j105409Scans,
		structures: [],
	},
	{ id: "Charmerout", name: "Charmerout", eve_system_id: 30004976, class: "HS", statics: [], scans: charmeroutScans, structures: [] },
	{
		id: "J150921",
		name: "J150921",
		eve_system_id: 31000453,
		class: "C2",
		statics: [
			{ wh_type: "O883", dest: "C3" },
			{ wh_type: "D845", dest: "HS" },
		],
		scans: j150921Scans,
		structures: [],
	},
	{
		id: "J134301",
		name: "J134301",
		eve_system_id: 31000722,
		class: "C3",
		statics: [{ wh_type: "N062", dest: "LS" }],
		scans: j134301Scans,
		structures: [],
	},
	{ id: "Hatakani", name: "Hatakani", eve_system_id: 30002764, class: "HS", statics: [], scans: hatakaniScans, structures: [] },
	{
		id: "J162332",
		name: "J162332",
		eve_system_id: 31001984,
		class: "C2",
		statics: [
			{ wh_type: "Z647", dest: "C1" },
			{ wh_type: "D845", dest: "HS" },
		],
		scans: j162332Scans,
		structures: [],
	},
	{ id: "Hurjafren", name: "Hurjafren", eve_system_id: 30002572, class: "HS", statics: [], scans: hurjafrenScans, structures: [] },
	{
		id: "J143517",
		name: "J143517",
		eve_system_id: 31000562,
		class: "C1",
		statics: [{ wh_type: "D845", dest: "HS" }],
		scans: j143517Scans,
		structures: [],
	},
	{
		id: "J100858",
		name: "J100858",
		eve_system_id: 31002024,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "C5" }],
		scans: j100858Scans,
		structures: [],
	},
	{
		id: "J010951",
		name: "J010951",
		eve_system_id: 31002579,
		class: "C6",
		statics: [
			{ wh_type: "H296", dest: "C5" },
			{ wh_type: "V911", dest: "NS" },
		],
		scans: j010951Scans,
		structures: [],
	},
	{
		id: "J211517",
		name: "J211517",
		eve_system_id: 31002517,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "C6" }],
		scans: j211517Scans,
		structures: [],
	},
	{
		id: "J145512",
		name: "J145512",
		eve_system_id: 31001904,
		class: "C6",
		statics: [{ wh_type: "V911", dest: "C6" }],
		scans: j145512Scans,
		structures: [],
	},
	{
		id: "J152912",
		name: "J152912",
		eve_system_id: 31002070,
		class: "C6",
		statics: [{ wh_type: "V911", dest: "C6" }],
		scans: j152912Scans,
		structures: [],
	},
	{ id: "R-ZUOL", name: "R-ZUOL", eve_system_id: 30002135, class: "NS", statics: [], scans: rzuolScans, structures: [] },
	{
		id: "J140717",
		name: "J140717",
		eve_system_id: 31001922,
		class: "C5",
		statics: [{ wh_type: "X877", dest: "C2" }],
		scans: j140717Scans,
		structures: [],
	},
	{ id: "Sarline", name: "Sarline", eve_system_id: 30003584, class: "LS", statics: [], scans: sarlineScans, structures: [] },
	{
		id: "J013070",
		name: "J013070",
		eve_system_id: 31002427,
		class: "C2",
		statics: [
			{ wh_type: "V911", dest: "C6" },
			{ wh_type: "N062", dest: "LS" },
		],
		scans: j013070Scans,
		structures: [],
	},
	{
		id: "J150606",
		name: "J150606",
		eve_system_id: 31000416,
		class: "C5",
		statics: [{ wh_type: "H296", dest: "C2" }],
		scans: j150606Scans,
		structures: [],
	},
	{
		id: "J152722",
		name: "J152722",
		eve_system_id: 31002103,
		class: "C6",
		statics: [{ wh_type: "V911", dest: "C4" }],
		scans: j152722Scans,
		structures: [],
	},
	{
		id: "J110413",
		name: "J110413",
		eve_system_id: 31000687,
		class: "C2",
		statics: [{ wh_type: "N062", dest: "LS" }],
		scans: j110413Scans,
		structures: [],
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
		// Fresh + stable — the CALM baseline: fat solid green line, no halo.
		id: "c-jita-j1",
		a: { system: "Jita", sig: { id: "ABC-001", type: "K162" } },
		b: { system: "J100001", sig: { id: "XYZ-100", type: "R943" } },
		mass: "fresh",
		ttl_remaining_min: 1400,
		eol: false,
	},
	{
		// Named (C247) on J100001, K162 on J100002. Half mass, < 4 h left → reduced
		// width + long dash + a gentle amber WARNING casing.
		// REVERSED orientation: K162 is on the upstream (J100001) side, the named hole
		// on J100002 — so the arrow points back UP-chain. (Direction is per-hole data,
		// not a uniform flow; ~40% of the fixture's holes read this way.)
		id: "c-j1-j2",
		a: { system: "J100001", sig: { id: "DEF-002", type: "K162" } },
		b: { system: "J100002", sig: { id: "XYZ-101", type: "C247" } },
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
		// REVERSED orientation: K162 upstream (J100004), named hole on J100005.
		id: "c-j4-j5",
		a: { system: "J100004", sig: { id: "VWX-301", type: "K162" } },
		b: { system: "J100005", sig: { id: "YZA-302", type: "M267" } },
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
		// renders. REVERSED orientation: K162 on the J-space side, named C729 in
		// Pochven — the arrow points back up-chain toward J100004.
		id: "c-j4-krirald",
		a: { system: "J100004", sig: { id: "TRG-401", type: "K162" } },
		b: { system: "Krirald", sig: { id: "TRG-402", type: "C729" } },
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
		// REVERSED orientation, so the two parallel holes between J100006↔J100007 point
		// OPPOSITE ways — a clean demo that direction is per-hole, not per-pair: K162 on
		// J100007, named Z142 on J100006.
		id: "c-j6-j7-b",
		a: { system: "J100006", sig: { id: "NOP-801", type: "K162" } },
		b: { system: "J100007", sig: { id: "QRS-802", type: "Z142" } },
		mass: "half",
		ttl_remaining_min: 1100,
		eol: false,
	},
	// ── Critical MASS × warning/critical TTL (down-chain off J100007) ────────────
	// With the dash texture dropped, TTL rides ONLY on the pulsing casing — these two
	// show a thin red (critical-mass) line wearing each pulse tier.
	{
		// CRITICAL mass + WARNING ttl (< 4 h): thin red line + a gentle AMBER pulse.
		// Named D364 on J100007, K162 on J100009.
		id: "c-j7-j9",
		a: { system: "J100007", sig: { id: "D364-901", type: "D364" } },
		b: { system: "J100009", sig: { id: "WRN-902", type: "K162" } },
		mass: "critical",
		ttl_remaining_min: 150,
		eol: false,
	},
	{
		// CRITICAL mass + CRITICAL ttl (< 1 h): thin red line + a deep RED breathing
		// pulse — the loudest combo. K162 on J100009, named N062 on J100010.
		id: "c-j9-j10",
		a: { system: "J100009", sig: { id: "CRT-903", type: "K162" } },
		b: { system: "J100010", sig: { id: "N062-904", type: "N062" } },
		mass: "critical",
		ttl_remaining_min: 40,
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
	// ── DEEP chain connections ───────────────────────────────────────────────
	// Edges fan outward from the root J172840. Named hole on the upstream side,
	// K162 on the downstream side (so arrows point back up-chain), with a couple of
	// mass / TTL states sprinkled in for variety. One k-space pair (Hurjafren ↔
	// J143517) reads as a magenta-ish EoL hole in the screenshot — modelled here as
	// a half-mass, < 4 h warning edge.
	{
		// Root WAT-512 (H296) on J172840, K162 on J120922 — sig_id matches the root
		// scan WAT-512.
		id: "c-deep-root-wat",
		a: { system: "J172840", sig: { id: "WAT-512", type: "H296" } },
		b: { system: "J120922", sig: { id: "SOH-872", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1400,
		eol: false,
	},
	{
		// Root SWE-200 (K162) on J172840, named H296 on J100858.
		id: "c-deep-root-swe",
		a: { system: "J172840", sig: { id: "SWE-200", type: "K162" } },
		b: { system: "J100858", sig: { id: "NBK-200", type: "H296" } },
		mass: "fresh",
		ttl_remaining_min: 1300,
		eol: false,
	},
	{
		// Root RMZ-780 (K162) on J172840, named X877 on J140717. Ageing out fast in the
		// screenshot (~6 min) → IMMINENT TTL, so this edge wears the loud red pulse.
		id: "c-deep-root-rmz",
		a: { system: "J172840", sig: { id: "RMZ-780", type: "K162" } },
		b: { system: "J140717", sig: { id: "JVK-855", type: "X877" } },
		mass: "half",
		ttl_remaining_min: 6,
		eol: true,
	},
	{
		// Root TAR-387 (C140) on J172840, K162 on Sarline (a low-sec exit).
		id: "c-deep-root-tar",
		a: { system: "J172840", sig: { id: "TAR-387", type: "C140" } },
		b: { system: "Sarline", sig: { id: "XVK-111", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1500,
		eol: false,
	},
	// WAT branch
	{
		id: "c-deep-wat-gsg",
		a: { system: "J120922", sig: { id: "OGJ-470", type: "E175" } },
		b: { system: "J153054", sig: { id: "RWR-551", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1200,
		eol: false,
	},
	{
		id: "c-deep-wat-dwm",
		a: { system: "J120922", sig: { id: "DWM-464", type: "M267" } },
		b: { system: "J113551", sig: { id: "RTU-152", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1100,
		eol: false,
	},
	{
		id: "c-deep-wat-zvd",
		a: { system: "J120922", sig: { id: "ZVD-547", type: "X877" } },
		b: { system: "J162332", sig: { id: "LBI-075", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 900,
		eol: false,
	},
	// DWM branch
	{
		id: "c-deep-dwm-tje",
		a: { system: "J113551", sig: { id: "TJE-744", type: "M267" } },
		b: { system: "J105409", sig: { id: "MMY-968", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1000,
		eol: false,
	},
	{
		id: "c-deep-dwm-rtu",
		a: { system: "J113551", sig: { id: "KLR-720", type: "X877" } },
		b: { system: "J150921", sig: { id: "RXD-181", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1050,
		eol: false,
	},
	{
		id: "c-deep-tje-mmy",
		a: { system: "J105409", sig: { id: "HGO-317", type: "D845" } },
		b: { system: "Charmerout", sig: { id: "JDX-488", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1440,
		eol: false,
	},
	{
		id: "c-deep-rtu-hfn",
		a: { system: "J150921", sig: { id: "WVM-085", type: "O883" } },
		b: { system: "J134301", sig: { id: "GUS-961", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1200,
		eol: false,
	},
	{
		id: "c-deep-rtu-vxq",
		a: { system: "J150921", sig: { id: "LCZ-027", type: "D845" } },
		b: { system: "Hatakani", sig: { id: "OCL-374", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1380,
		eol: false,
	},
	// ZVD branch (the k-space pair with the magenta link in the screenshot)
	{
		id: "c-deep-zvd-lwp",
		a: { system: "J162332", sig: { id: "RKP-689", type: "D845" } },
		b: { system: "Hurjafren", sig: { id: "IGS-363", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1300,
		eol: false,
	},
	{
		id: "c-deep-lwp-ydo",
		a: { system: "Hurjafren", sig: { id: "SWE-233", type: "Z647" } },
		b: { system: "J143517", sig: { id: "RTI-914", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 180,
		eol: false,
	},
	// SWE branch
	{
		id: "c-deep-swe-qwg",
		a: { system: "J100858", sig: { id: "QWQ-060", type: "H296" } },
		b: { system: "J010951", sig: { id: "ZZX-559", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1100,
		eol: false,
	},
	{
		id: "c-deep-qwg-vog",
		a: { system: "J010951", sig: { id: "RJP-460", type: "H296" } },
		b: { system: "J211517", sig: { id: "ODC-412", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1000,
		eol: false,
	},
	{
		id: "c-deep-qwg-qps",
		a: { system: "J010951", sig: { id: "VOP-803", type: "H296" } },
		b: { system: "J145512", sig: { id: "GBZ-653", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 950,
		eol: false,
	},
	{
		id: "c-deep-qwg-rjp",
		a: { system: "J010951", sig: { id: "XEC-225", type: "H296" } },
		b: { system: "J152912", sig: { id: "WLF-919", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1150,
		eol: false,
	},
	{
		id: "c-deep-qwg-xxj",
		a: { system: "J010951", sig: { id: "AWF-387", type: "V911" } },
		b: { system: "R-ZUOL", sig: { id: "RVS-284", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 160,
		eol: false,
	},
	// TAR branch
	{
		id: "c-deep-tar-zwz",
		a: { system: "Sarline", sig: { id: "VYR-775", type: "N062" } },
		b: { system: "J013070", sig: { id: "CIT-497", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1250,
		eol: false,
	},
	{
		id: "c-deep-zwz-odc",
		a: { system: "J013070", sig: { id: "SRB-511", type: "V911" } },
		b: { system: "J150606", sig: { id: "DLD-385", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1080,
		eol: false,
	},
	{
		id: "c-deep-zwz-rwn",
		a: { system: "J013070", sig: { id: "ORX-926", type: "V911" } },
		b: { system: "J152722", sig: { id: "HFZ-857", type: "K162" } },
		mass: "fresh",
		ttl_remaining_min: 1170,
		eol: false,
	},
	{
		id: "c-deep-zwz-rop",
		a: { system: "J013070", sig: { id: "ENV-256", type: "N062" } },
		b: { system: "J110413", sig: { id: "KFP-798", type: "K162" } },
		mass: "half",
		ttl_remaining_min: 800,
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
		// A wide, deeply-branched chain modelled on a Wanderer screenshot, rooted
		// at J172840. Reads best under the LR layout (the screenshot is left-to-right).
		{ id: "deep", label: "DEEP", root: "J172840" },
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
	ghostSystems: [
		{ id: "J199999", name: "J199999", eve_system_id: null, class: "C2", statics: [], scans: [], structures: [] },
	],
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
		system: {
			id: "J100008",
			name: "J100008",
			eve_system_id: 31000008,
			class: "C4",
			statics: [],
			scans: [],
			structures: [],
		},
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
			eve_system_id: 31009999,
			class: "C2",
			statics: [{ wh_type: "H121", dest: "C1" }],
			scans: [],
			structures: [],
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
