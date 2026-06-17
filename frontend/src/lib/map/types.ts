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

/** Wormhole / J-space class. C1–C6 are the wormhole classes; the security tiers
 *  (HS/LS/NS) describe the known-space anchor a chain hangs off. */
export type SystemClass = 'C1' | 'C2' | 'C3' | 'C4' | 'C5' | 'C6' | 'HS' | 'LS' | 'NS';

/** A static wormhole a system always spawns (e.g. a C5 system with a C5+HS pair).
 *  `dest` is the static's destination class; `code` is the in-game signature-ish
 *  shorthand the badge renders (e.g. `C5a`, `HSa`). */
export interface SystemStatic {
	code: string;
	dest: SystemClass;
}

/** A system as the combined graph knows it. No coordinates — placement is separate. */
export interface System {
	/** Stable identity (J-code for wormholes, system name for k-space). Doubles as
	 *  the node id throughout layout / placement / reconcile. */
	id: string;
	/** Display name; for wormholes this is usually the same as `id` (the J-code). */
	name: string;
	class: SystemClass;
	statics: SystemStatic[];
}

/** Connection mass state. Drives both a colour token and a TEXT cue on the edge
 *  label — mass is never colour-only (Fork 3). */
export type Mass = 'fresh' | 'half' | 'critical';

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
	/** End-of-life: the wormhole is in its final ~4h window. Carries a `⚠` glyph
	 *  on the label (non-colour) plus a pulse decoration. */
	eol: boolean;
}

/** The K162 end of a connection — the endpoint the direction arrow points TO —
 *  or `null` when neither side's type is known. Derived: the K162 side is the
 *  one typed `K162`; if only the NAMED side is known, the K162 is the other end.
 *  (K162 and named are complementary, so one known type orients the arrow.) */
export function k162End(conn: Connection): 'a' | 'b' | null {
	const aType = conn.a.sig?.type ?? null;
	const bType = conn.b.sig?.type ?? null;
	// a is K162, or b is a known NAMED side ⇒ the K162 is the a end.
	if (aType === 'K162' || (bType !== null && bType !== 'K162')) return 'a';
	// b is K162, or a is a known NAMED side ⇒ the K162 is the b end.
	if (bType === 'K162' || (aType !== null && aType !== 'K162')) return 'b';
	return null; // both ends unknown → direction undetermined
}

/** A view onto the combined graph: a named tab anchored at one or more roots.
 *  Render = systems reachable from `roots` over live connections. The wildcard
 *  tab (`isWildcard`) shows everything regardless of roots (e.g. eve-scout). */
export interface Tab {
	id: string;
	label: string;
	/** Root SET — multi-root is first-class. Empty + `isWildcard` ⇒ show all. */
	roots: string[];
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

/** Layout direction for the one-shot "redo layout" action (Fork 2). The four
 *  cardinal flows rank away from the roots in that screen direction; `radial`
 *  fans ranks in concentric rings around the root. */
export type LayoutDirection = 'LR' | 'RL' | 'TB' | 'BT' | 'radial';

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
	| { kind: 'add-system'; system: System; anchor: string; connection: Connection }
	| { kind: 'add-connection'; connection: Connection }
	| { kind: 'remove-system'; id: string }
	| { kind: 'remove-connection'; id: string };
