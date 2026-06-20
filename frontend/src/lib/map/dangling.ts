/**
 * Dangling (unconfirmed) wormhole stubs.
 *
 * A scanned wormhole signature (`site_type: 'Wormhole'`) records that a hole
 * EXISTS in a system, but until someone jumps it (or scans the far side) there is
 * no {@link Connection} reaching a destination system. The map should still SHOW
 * that hole: a faint stub node on the far end, so a scanned-but-unjumped wormhole
 * reads as "there is a connection here we haven't followed yet" instead of
 * vanishing until a connection is created.
 *
 * This module DERIVES those stubs from the graph (pure, no state): for every
 * wormhole scan whose `(system, sig_id)` is not referenced by any connection
 * endpoint, it synthesises a stub System + a Connection from the source system to
 * it. Folding the result into the combined graph lets the stubs flow through
 * reachability + layout + edge rendering with no special-casing — they rank one
 * step out from their source like any neighbour. The synthetic ids are namespaced
 * (`dangling:<system>:<sig_id>`) so they never collide with real systems/conns and
 * the node renderer can tell a stub apart (the `DANGLING_PREFIX`).
 *
 * The stub's destination CLASS is inferred from the wormhole TYPE code where we
 * know it ({@link WH_DEST_CLASS}); a K162 (incoming, by definition unknown) or an
 * unidentified/uncommon code leaves it unknown and the node shows a bare `?`.
 */

import type { CombinedGraph, Connection, Mass, System, SystemClass } from './types';

/** Synthetic-id namespace for a dangling stub system + its connection. */
export const DANGLING_PREFIX = 'dangling:';

/** True for a node/connection id this module minted. */
export function isDanglingId(id: string): boolean {
	return id.startsWith(DANGLING_PREFIX);
}

/**
 * Destination system class for a (named) wormhole type code, where it is known
 * and single-valued. K162 is an INCOMING hole — its origin is unknown by
 * definition, so it is deliberately absent (→ `?`). Codes we can't confidently map
 * (small-ship holes, etc.) are absent too; the stub then shows a bare `?` rather
 * than a guess. Seeded with the codes the prototype fixture exercises; extend as
 * the real wormhole-type catalogue lands. (Verified against EVE University's
 * wormhole tables: B274→HS, O477→C3, R474→C6.)
 */
export const WH_DEST_CLASS: Record<string, SystemClass> = {
	// k-space destinations
	B274: 'HS',
	D845: 'HS',
	N062: 'LS',
	S199: 'NS',
	// w-space destinations
	H121: 'C1',
	Z647: 'C1',
	C247: 'C2',
	X877: 'C2',
	O477: 'C3',
	O883: 'C3',
	M267: 'C3',
	Y683: 'C4',
	H296: 'C5',
	V911: 'C6',
	R474: 'C6'
};

/** Mass a stub connection carries. A scanned-but-unjumped hole's mass is unknown;
 *  the prototype renders it `fresh` (the calmest encoding) — it is a placeholder,
 *  not an at-risk hole. */
const STUB_MASS: Mass = 'fresh';

/**
 * Derive the dangling stubs for a graph: a stub System + a Connection per wormhole
 * scan that no connection endpoint references. Returns empty arrays when there are
 * none. Pure — callers union the result into the graph they render.
 */
export function danglingStubs(graph: CombinedGraph): {
	systems: System[];
	connections: Connection[];
	/** Stub system id → inferred destination class, or `null` when the wormhole type
	 *  is unknown/unmappable (K162, unidentified) — the node then shows a bare `?`. */
	dest: Map<string, SystemClass | null>;
} {
	// Every (system, sig_id) a connection already accounts for.
	const referenced = new Set<string>();
	for (const c of graph.connections) {
		if (c.a.sig) referenced.add(`${c.a.system}|${c.a.sig.id}`);
		if (c.b.sig) referenced.add(`${c.b.system}|${c.b.sig.id}`);
	}

	const systems: System[] = [];
	const connections: Connection[] = [];
	const dest = new Map<string, SystemClass | null>();

	for (const sys of graph.systems) {
		for (const scan of sys.scans) {
			if (scan.site_type !== 'Wormhole') continue;
			if (referenced.has(`${sys.id}|${scan.sig_id}`)) continue;

			const stubId = `${DANGLING_PREFIX}${sys.id}:${scan.sig_id}`;
			// Infer the destination class from the wormhole type where known; else the
			// stub class is unknown and the node shows a bare `?` (we carry C1 only to
			// satisfy the required `class` field — the renderer suppresses it for a
			// dangling node and reads `wh_type` instead).
			const destClass = (scan.wh_type && WH_DEST_CLASS[scan.wh_type]) || null;
			dest.set(stubId, destClass);
			systems.push({
				id: stubId,
				// A friendly placeholder name (the namespaced id stays the stable node
				// id); the dangling renderer shows `? → <dest>` rather than this.
				name: '?',
				eve_system_id: null,
				// Placeholder to satisfy the type; the dangling renderer ignores it and
				// reads the `dest` map instead.
				class: destClass ?? 'C1',
				statics: [],
				scans: [],
				structures: []
			});
			// A connection from the source system to its stub, carrying the scanned
			// sig on the source end so the edge's sig pill still reads. The far end has
			// no sig (it's unscanned) → direction stays undetermined (no arrow).
			connections.push({
				id: `${DANGLING_PREFIX}${sys.id}:${scan.sig_id}`,
				a: { system: sys.id, sig: { id: scan.sig_id, type: scan.wh_type } },
				b: { system: stubId, sig: null },
				mass: STUB_MASS,
				ttl_remaining_min: 1440,
				eol: false
			});
		}
	}

	return { systems, connections, dest };
}
