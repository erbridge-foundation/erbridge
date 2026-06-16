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

/** A live connection between two systems. Directed by `origin` (the side the
 *  chain was scanned *from*) but rendered as an undirected link; reachability
 *  walks it both ways. */
export interface Connection {
	id: string;
	/** The two endpoint system ids. `origin` is whichever of these the connection
	 *  was scanned from (presentation hint only — reachability is symmetric). */
	source: string;
	target: string;
	origin: string;
	/** Wormhole type code shown on the edge label (e.g. `C247`, `D845`, `K162`). */
	wh_type: string;
	mass: Mass;
	/** End-of-life: the wormhole is in its final ~4h window. Carries a `⚠` glyph
	 *  on the label (non-colour) plus a pulse decoration. */
	eol: boolean;
	/** Signature ids at each end, as scanned. Optional — not every link has both
	 *  sides bookmarked yet. */
	sig_source?: string;
	sig_target?: string;
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
