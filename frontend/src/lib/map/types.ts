/**
 * Position-less combined-graph contract for the map canvas (sandbox).
 *
 * The spine (see build-map-canvas-prototype/design.md) keeps three axes apart:
 *   EXISTENCE — this graph. Node/edge existence is a pure function of it,
 *               NEVER derived from placement.
 *   PLACEMENT — pure presentation, lives elsewhere (placement.ts / localStorage).
 *   STYLE     — the custom svelte-flow node/edge components.
 *
 * Nothing here carries an (x, y): positions come from the layout seed and/or
 * saved placement, overlaid at render time. A `System` describes *what is*, not
 * *where it sits*.
 */

/** Wormhole / J-space class. C1–C6 are the wormhole classes; the k-space tiers
 *  (HS/LS/NS) describe the known-space anchor a chain hangs off. `P` is Pochven
 *  (Triglavian space) — its OWN distinct space type, not null- or low-sec: it
 *  displays a null-ish security status but is a separate region with its own
 *  access + connectivity rules, so the map tracks it as a first-class tier. `D` is
 *  a Drifter wormhole */
export type SystemClass =
	| "C1"
	| "C2"
	| "C3"
	| "C4"
	| "C5"
	| "C6"
	| "HS"
	| "LS"
	| "NS"
	| "P"
	| "D";

/** A static wormhole a system always spawns (e.g. a C5 system with a C5+HS pair).
 *  `dest` is the static's destination class (HS/LS/NS/C1–C6) — the only thing the
 *  map surfaces for now. `wh_type` is the actual wormhole TYPE code (e.g. `C008`,
 *  `N062`), kept for a later piece of work (scanning signatures and offering which
 *  wormhole type a static is) but NOT displayed on the node yet. */
export interface SystemStatic {
	wh_type: string;
	dest: SystemClass;
}

/**
 * Provenance / tracking metadata carried by every map record we author (scanned
 * signatures, structures). Records the who/when of creation and last update so the
 * sidebar can show recency and the paste-ingest pipeline (a later phase) can stamp
 * updates in place.
 *
 * DECISION: every datetime in the model is an ISO-8601 string in **UTC**. The
 * `*_by` fields are EVE character ids (numeric — EVE's id space is numeric); name
 * resolution for display is a render-time concern, not stored here.
 */
export interface TrackingMeta {
	created_at: string;
	created_by: number;
	updated_at: string;
	updated_by: number;
}

/** Probe-scanner CATEGORY column — the scanner's top-level grouping, DISTINCT from
 *  the per-site classification (`site_type`). The earlier prototype conflated the
 *  two; they are separate axes. */
export type ScanGroup =
	| "Cosmic Signature"
	| "Cosmic Anomaly"
	| "Ship"
	| "Structure";

/**
 * A scanned signature / anomaly in a system — the canonical EVE Probe Scanner row,
 * plus our {@link TrackingMeta}. The wormhole TYPE that orients a connection lives
 * on the scan (`wh_type`), not the connection.
 *
 * Notes:
 *   - `group` (category) and `site_type` (classification) are DISTINCT axes.
 *   - `distance_au` from the raw scanner export is deliberately OMITTED — it drifts
 *     and is never used for identity (per the schema notes).
 *   - `strength_pct` is DROPPED (user decision): scan progression is read off
 *     `site_type` / `name` alone (see the scanIs* helpers), and recency comes from
 *     `updated_at`, so the percentage carries no display weight here.
 *   - `sig_id` is the STABLE primary key across the 0→100% scan; the paste-ingest
 *     pipeline reconciles repeat scans in place by it.
 */
export interface ScanResult extends TrackingMeta {
	/** In-game signature id, e.g. `ABC-123`. Stable across the scan; primary key. */
	sig_id: string;
	/** Scanner category (not the classification). */
	group: ScanGroup;
	/** Per-site classification: 'Wormhole' | 'Data Site' | 'Gas Site' | 'Ore Site' |
	 *  'Relic Site' | 'Citadel' | … ; `null` until the site is identified. Map search
	 *  (a later phase) keys off this ("gas" → site_type 'Gas Site', etc.). */
	site_type: string | null;
	/** Specific site name; `null` until the scan resolves it. */
	name: string | null;
	/** Wormhole TYPE code (`K162`, `H296`, …) — the one field BEYOND the raw scanner
	 *  schema. The scanner only reports name 'Unstable Wormhole'; the map needs the
	 *  code to derive direction. `null` unless this is a typed wormhole sig. */
	wh_type: string | null;
}

/** A scan that hasn't been classified yet (site_type still unknown). */
export function scanIsUnknown(r: ScanResult): boolean {
	return r.site_type === null;
}
/** A scan classified but not yet fully resolved (type known, name not). */
export function scanIsPartial(r: ScanResult): boolean {
	return r.site_type !== null && r.name === null;
}
/** A fully-resolved scan (named). With strength_pct dropped, a name is the marker
 *  of a completed scan. */
export function scanIsResolved(r: ScanResult): boolean {
	return r.name !== null;
}

/** Where a {@link Structure} record came from. Only `name` is reliable across all
 *  three sources (see {@link Structure}). */
export type StructureSource = "scanner" | "dscan" | "overview";

/** A reinforcement / anchoring timer on a structure. Carried only by overview-paste
 *  records (and not always); d-scan and scanner sources never have one. `ends_at`
 *  is ISO-8601 UTC. */
export interface StructureTimer {
	state: "reinforced" | "anchoring" | "unanchoring";
	ends_at: string;
}

/**
 * A structure in a system — a DISTINCT first-class entity, NOT merely a
 * {@link ScanResult}. Structures arrive THREE ways with very different fields, so
 * almost everything is nullable; `name` is the only field present in all sources.
 *
 *   - `scanner`  : a probe-scan row (group 'Structure'); has `sig_id`, `hull`.
 *   - `dscan`    : a d-scan paste row; has `type_id`, `hull`; NO `sig_id`, NO owner.
 *   - `overview` : an overview-selected paste; `name` (+ sometimes `owner`/`timer`)
 *                  ONLY — no `type_id`, no `hull`, no `sig_id`.
 *
 * Identity = `name` (reconcile by it), with `sig_id` agreement CONFIRMING a match
 * when both records carry one. Carries the SAME {@link TrackingMeta} as a scan.
 */
export interface Structure extends TrackingMeta {
	id: string;
	/** The only field present across all three sources. */
	name: string;
	/** EVE type id of the hull — from d-scan. */
	type_id: number | null;
	/** Hull name ('Astrahus' / 'Fortizar' / …) — d-scan or scanner. */
	hull: string | null;
	/** Owning corp/alliance — manual or overview; never d-scan or scanner. */
	owner: string | null;
	/** Links to a {@link ScanResult} when the structure was also probe-scanned. */
	sig_id: string | null;
	/** Reinforcement/anchoring timer — overview-paste only, sometimes. */
	timer: StructureTimer | null;
	source: StructureSource;
}

/**
 * D-scan structure hull allow-list. A d-scan paste is mostly noise (moons, planets,
 * POCOs, the star, containers); the paste-ingest pipeline (a later phase) keeps only
 * rows whose `type_text` hull is in this set and discards the rest. Declared here as
 * the documented source of truth even though nothing consumes it yet — the parser
 * will. POCOs (Customs Offices) are deliberately a SEPARATE class, tracked later.
 */
export const STRUCTURE_HULL_ALLOWLIST: readonly string[] = [
	// Citadels
	"Astrahus",
	"Fortizar",
	"Keepstar",
	// Engineering complexes
	"Raitaru",
	"Azbel",
	"Sotiyo",
	// Refineries
	"Athanor",
	"Tatara",
	// Faction / special (extend as samples arrive)
	"Palatine Keepstar",
];

/** A system as the combined graph knows it. No coordinates — placement is separate. */
export interface System {
	/** Stable identity (J-code for wormholes, system name for k-space). Doubles as
	 *  the node id throughout layout / placement / reconcile. */
	id: string;
	/** Display name; for wormholes this is usually the same as `id` (the J-code). */
	name: string;
	/** EVE ESI numeric solar-system id (`solar_system_id`). This is the STABLE key
	 *  the real backend / external tools (eve-scout, Wanderer) join on — `id`/`name`
	 *  is the display key, this is the universe key. `null` when the system hasn't
	 *  been resolved to a real id yet (e.g. a hand-placed ghost). Carried so a later
	 *  phase can reconcile pasted signature data (keyed on `solar_system_id`) onto
	 *  the right node. */
	eve_system_id: number | null;
	class: SystemClass;
	statics: SystemStatic[];
	/** In-system scanned signatures + anomalies (incl. scanned structures, which
	 *  also appear as first-class {@link Structure}s). A wormhole scan
	 *  (`site_type: 'Wormhole'`) is what a {@link Connection} references by system +
	 *  sig_id. */
	scans: ScanResult[];
	/** First-class structures in the system (multi-source — see {@link Structure}). */
	structures: Structure[];
}

/** Connection mass state. Drives line THICKNESS + colour, plus a TEXT cue on the
 *  edge label — mass is never colour-only (Fork 3). Thresholds follow the in-game
 *  stability text: fresh > 50%, reduced (half) < 50%, critical < 10%. */
export type Mass = "fresh" | "half" | "critical";

/**
 * A wormhole type's MAXIMUM stable lifetime, in minutes, as the eve-scout import
 * gives it (`max_stable_time`). The observed value set is:
 *
 *   [0, 270, 720, 960, 1440, 2880]
 *
 * 0 is the special "no fixed lifetime" marker some types carry; the rest are
 * 4.5 h / 12 h / 16 h / 24 h / 48 h. A type's max is the CEILING — a hole opens
 * with this much life and only ever counts down. The map cares about how much is
 * LEFT (see {@link TtlState}), not the ceiling, but the ceiling is what the
 * backend stores per type, so the prototype carries it for realism.
 *
 * Note 270 (= 4.5 h) is the frigate-hole ceiling; a standard hole's is 240
 * (4 h). The in-game stability TEXT only ever rounds to "less than 4 hours"
 * regardless — the exact ceiling (4 h vs 4.5 h) is a per-type attribute people
 * will be able to pick later, NOT a display bucket. The {@link TtlState} buckets
 * below mirror the rounded in-game text, not the ceiling.
 */
export type MaxStableMin = 0 | 270 | 720 | 960 | 1440 | 2880;

/** The known `max_stable_time` values, smallest first. The 0 marker sorts first
 *  but is the "unknown/no fixed lifetime" case, not the shortest-lived. */
export const MAX_STABLE_MIN: readonly MaxStableMin[] = [
	0, 270, 720, 960, 1440, 2880,
];

/**
 * The DISPLAYED time-to-live bucket — derived from how many minutes of life a
 * connection has LEFT, independent of its type's ceiling. These are the UX
 * buckets the edge encoding draws (dash + glyph + alert); they are NOT the raw
 * EVE lifetime, which is a continuous countdown.
 *
 *   - `stable`   : anything above 4 h left.
 *   - `lt4h`     : under 4 h left.
 *   - `lt1h`     : under 1 h left — the actionable "act now" window.
 *   - `imminent` : minutes left, effectively too late to use.
 *
 * We track all FOUR as distinct states (the model/enum cares about the
 * difference, e.g. for sorting or future tooling), but the MAP visual collapses
 * them to three (see {@link TtlVisual}): `lt1h` and `imminent` render the SAME
 * loud critical state, because by the time it's imminent the urgency message is
 * unchanged — there's nothing new to say, it's just past saving.
 *
 * DECISION: these spec buckets are authoritative for OUR tool, NOT the in-game
 * lifetime text. EVE currently shows four states ("< 1 day / < 4 h / < 1 h /
 * Expired, closure imminent"), but that is CCP's UI and can change; we don't
 * bind to it. In particular EVE's calm top state is "< 1 day" whereas ours is
 * "anything above 4 h" — by design, so a healthy hole reads calm (no glyph,
 * solid line). Only the under-4-h escalation carries cues.
 */
export type TtlState = "stable" | "lt4h" | "lt1h" | "imminent";

/**
 * The three VISUAL urgency tiers the edge encoding actually draws. The four
 * {@link TtlState} buckets collapse onto these for rendering:
 *
 *   stable            → `calm`     (solid line, no glyph, no alert)
 *   lt4h              → `warning`  (amber dash + clock glyph)
 *   lt1h, imminent    → `critical` (red dash + alert glyph + red breathing halo)
 *
 * Mass-critical also forces at least `critical` on the alert layer independently
 * of TTL (a near-collapse-by-mass hole is its own emergency); see resolveAlert.
 */
export type TtlVisual = "calm" | "warning" | "critical";

/** Collapse a four-state {@link TtlState} onto the three visual tiers the map
 *  draws. `lt1h` and `imminent` are the SAME critical visual (see TtlState). */
export function ttlVisual(state: TtlState): TtlVisual {
	switch (state) {
		case "stable":
			return "calm";
		case "lt4h":
			return "warning";
		case "lt1h":
		case "imminent":
			return "critical";
	}
}

/** Bucket remaining-minutes into a {@link TtlState}. Thresholds are the chosen UX
 *  buckets (4 h / 1 h / imminent); confirm against real EVE EOL mechanics before
 *  the backend hardcodes them (see the edge-encoding spec's open questions). */
export function ttlState(remainingMin: number): TtlState {
	if (remainingMin < 15) return "imminent";
	if (remainingMin < 60) return "lt1h";
	if (remainingMin < 240) return "lt4h";
	return "stable";
}

/** A scanned signature at a connection endpoint. The wormhole TYPE lives on the
 *  signature, not the connection: a hole is `K162` on one side and a named code
 *  (`H296`, `C247`, …) on the other. `type` may be unknown (sig bookmarked but
 *  not yet identified) — modelled as `null`. */
export interface Signature {
	/** In-game signature id, e.g. `ABC-123`. */
	id: string;
	/** The wormhole type code, or `null` if scanned-but-unidentified. */
	type: string | null;
}

/** One end of a connection: the system it's in, and the signature there (or
 *  `null` when that side hasn't been scanned/bookmarked yet — the `???` case). */
export interface ConnectionEndpoint {
	system: string;
	sig: Signature | null;
}

/**
 * A live connection between two systems, modelled as a pair of endpoints:
 *   sys_a → sig_a  < conn >  sig_b → sys_b
 *
 * The wormhole type lives on each endpoint's signature, so DIRECTION is derived,
 * never stored: the arrow points toward the `K162` end (equivalently, away from
 * the named end). One known side is enough to orient it (K162 and named are
 * complementary); only when BOTH sides are unknown is the direction undetermined.
 * Rendered as an undirected link for reachability — that walks both ways.
 */
export interface Connection {
	id: string;
	a: ConnectionEndpoint;
	b: ConnectionEndpoint;
	mass: Mass;
	/**
	 * Minutes of life this wormhole has LEFT (a countdown). The edge encoding
	 * buckets it into a {@link TtlState} (see {@link ttlState}) for dash/glyph/
	 * alert. In the real backend this is derived from `opened_at + max_stable_time`
	 * minus now; the prototype carries a literal value per fixture connection.
	 */
	ttl_remaining_min: number;
	/**
	 * End-of-life: the wormhole is in its final window. Retained as a convenience
	 * flag, but it is now DERIVED from TTL (`ttlState() === 'imminent'`) rather
	 * than an independent input — the fixture sets it consistently and the
	 * encoding reads TTL, not this. Kept so existing call sites (marker colour,
	 * sr-text) keep compiling during the prototype.
	 */
	eol: boolean;
}

/** The K162 end of a connection — the endpoint the direction arrow points TO —
 *  or `null` when neither side's type is known. Derived: the K162 side is the
 *  one typed `K162`; if only the NAMED side is known, the K162 is the other end.
 *  (K162 and named are complementary, so one known type orients the arrow.) */
export function k162End(conn: Connection): "a" | "b" | null {
	const aType = conn.a.sig?.type ?? null;
	const bType = conn.b.sig?.type ?? null;
	// a is K162, or b is a known NAMED side ⇒ the K162 is the a end.
	if (aType === "K162" || (bType !== null && bType !== "K162")) return "a";
	// b is K162, or a is a known NAMED side ⇒ the K162 is the b end.
	if (bType === "K162" || (aType !== null && aType !== "K162")) return "b";
	return null; // both ends unknown → direction undetermined
}

/** A view onto the combined graph: a named tab anchored at a single root.
 *  Render = systems reachable from `root` over live connections. A new root
 *  system means a new tab — multi-root was dropped as unnecessary. The wildcard
 *  tab (`isWildcard`) shows everything regardless of `root` (e.g. eve-scout);
 *  its `root` is ignored (conventionally empty). */
export interface Tab {
	id: string;
	label: string;
	/** The single anchor system. Ignored when `isWildcard`. */
	root: string;
	isWildcard?: boolean;
}

/** The server's view: position-less systems + connections. Existence truth. */
export interface CombinedGraph {
	systems: System[];
	connections: Connection[];
	tabs: Tab[];
}

/** Client-local additions not yet confirmed by the server (e.g. a right-click
 *  "add system" ghost). Rendered as the union with server state; a system here
 *  is dropped once server state confirms it (reconcile.ts). */
export interface LocalState {
	/** Locally-added systems awaiting a real connection from the server. */
	ghostSystems: System[];
	/** Locally-added connections (rare in the sandbox; kept for symmetry). */
	ghostConnections: Connection[];
}

/** A 2-D placement. The unit of the layout seed and the saved overlay. */
export interface XY {
	x: number;
	y: number;
}

/** id → position. What layout produces and placement persists. */
export type Positions = Record<string, XY>;

/** Layout direction for the one-shot "redo layout" action (Fork 2). Each cardinal
 *  flow ranks away from the roots in that screen direction. */
export type LayoutDirection = "LR" | "RL" | "TB" | "BT";

/**
 * A live update from the server, modelled as the SSE event the real backend will
 * push (the sandbox replays a scripted list of these — see the fixture). The map
 * is laid out ONCE on initial load; thereafter the graph only ever changes
 * through these discrete events, and each is placed incrementally — never a
 * whole-map re-layout (see the incremental-placement model in design.md).
 *
 *   - `add-system`     : a system entered the graph, reached via `anchor` (an
 *                        existing system). Placed one flow-step out from the
 *                        anchor, then collisions are resolved across the graph.
 *   - `add-connection` : a new wormhole between two already-present systems.
 *   - `remove-system`  : a system left the graph (its edges drop with it).
 *   - `remove-connection` : a wormhole collapsed.
 */
export type MapEvent =
	| {
			kind: "add-system";
			system: System;
			anchor: string;
			connection: Connection;
	  }
	| { kind: "add-connection"; connection: Connection }
	| { kind: "remove-system"; id: string }
	| { kind: "remove-connection"; id: string };
