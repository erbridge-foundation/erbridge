import { describe, it, expect } from 'vitest';
import { danglingStubs, isDanglingId, DANGLING_PREFIX } from './dangling';
import type { CombinedGraph, Connection, ScanResult, System } from './types';

const meta = {
	created_at: '2026-06-19T00:00:00Z',
	created_by: 0,
	updated_at: '2026-06-19T00:00:00Z',
	updated_by: 0
};

function whScan(sig_id: string, wh_type: string | null): ScanResult {
	return {
		sig_id,
		group: 'Cosmic Signature',
		site_type: 'Wormhole',
		name: 'Unstable Wormhole',
		wh_type,
		...meta
	};
}
function otherScan(sig_id: string): ScanResult {
	return { sig_id, group: 'Cosmic Anomaly', site_type: 'Gas Site', name: 'Gas', wh_type: null, ...meta };
}

function sys(id: string, scans: ScanResult[] = []): System {
	return { id, name: id, eve_system_id: null, class: 'C2', statics: [], scans, structures: [] };
}
function conn(id: string, a: string, aSig: string | null, b: string): Connection {
	return {
		id,
		a: { system: a, sig: aSig ? { id: aSig, type: 'K162' } : null },
		b: { system: b, sig: null },
		mass: 'fresh',
		ttl_remaining_min: 1440,
		eol: false
	};
}

describe('danglingStubs', () => {
	it('mints a stub system + connection for an unreferenced wormhole scan', () => {
		const graph: CombinedGraph = {
			systems: [sys('A', [whScan('ABC-123', 'R474')])],
			connections: [],
			tabs: []
		};
		const { systems, connections, dest } = danglingStubs(graph);
		expect(systems).toHaveLength(1);
		expect(connections).toHaveLength(1);

		const stubId = `${DANGLING_PREFIX}A:ABC-123`;
		expect(systems[0].id).toBe(stubId);
		expect(isDanglingId(systems[0].id)).toBe(true);
		// The connection runs from the source system (carrying the scanned sig) to the
		// stub (far end unscanned → no sig).
		expect(connections[0].a).toMatchObject({ system: 'A', sig: { id: 'ABC-123', type: 'R474' } });
		expect(connections[0].b).toMatchObject({ system: stubId, sig: null });
		// R474 → C6 is a known destination.
		expect(dest.get(stubId)).toBe('C6');
	});

	it('does NOT mint a stub when a connection already references the (system, sig)', () => {
		const graph: CombinedGraph = {
			systems: [sys('A', [whScan('ABC-123', 'R474')]), sys('B')],
			connections: [conn('ab', 'A', 'ABC-123', 'B')],
			tabs: []
		};
		expect(danglingStubs(graph).systems).toHaveLength(0);
	});

	it('ignores non-wormhole scans', () => {
		const graph: CombinedGraph = {
			systems: [sys('A', [otherScan('GAS-001')])],
			connections: [],
			tabs: []
		};
		expect(danglingStubs(graph).systems).toHaveLength(0);
	});

	it('leaves the destination unknown (null) for an unmappable / K162 hole', () => {
		const graph: CombinedGraph = {
			systems: [sys('A', [whScan('K16-001', 'K162'), whScan('UNK-001', null)])],
			connections: [],
			tabs: []
		};
		const { dest } = danglingStubs(graph);
		expect(dest.get(`${DANGLING_PREFIX}A:K16-001`)).toBeNull();
		expect(dest.get(`${DANGLING_PREFIX}A:UNK-001`)).toBeNull();
	});

	it('returns empty (and a usable empty dest map) when there are no wormhole scans', () => {
		const out = danglingStubs({ systems: [sys('A')], connections: [], tabs: [] });
		expect(out.systems).toHaveLength(0);
		expect(out.connections).toHaveLength(0);
		expect(out.dest.size).toBe(0);
	});
});
