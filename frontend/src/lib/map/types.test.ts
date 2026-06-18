import { describe, it, expect } from 'vitest';
import { k162End, scanIsUnknown, scanIsPartial, scanIsResolved } from './types';
import type { Connection, ScanResult, Signature } from './types';

function conn(aType: string | null | undefined, bType: string | null | undefined): Connection {
	const sig = (t: string | null | undefined): Signature | null =>
		t === undefined ? null : { id: 'X', type: t };
	return {
		id: 'c',
		a: { system: 'A', sig: sig(aType) },
		b: { system: 'B', sig: sig(bType) },
		mass: 'fresh',
		ttl_remaining_min: 1440,
		eol: false
	};
}

describe('k162End — derived connection direction (arrow points to the K162 end)', () => {
	it('points to the K162 end when that side is typed K162', () => {
		expect(k162End(conn('H296', 'K162'))).toBe('b');
		expect(k162End(conn('K162', 'H296'))).toBe('a');
	});

	it('infers from the NAMED side alone (far end unscanned)', () => {
		// a is named ⇒ the K162 must be the b end, even though b is unknown.
		expect(k162End(conn('H296', undefined))).toBe('b');
		// b is named ⇒ K162 is the a end.
		expect(k162End(conn(undefined, 'C247'))).toBe('a');
	});

	it('infers from the K162 side alone (near end unscanned)', () => {
		expect(k162End(conn(undefined, 'K162'))).toBe('b');
		expect(k162End(conn('K162', undefined))).toBe('a');
	});

	it('is undetermined when both ends are unknown', () => {
		expect(k162End(conn(undefined, undefined))).toBeNull(); // no sigs at all
		expect(k162End(conn(null, null))).toBeNull(); // scanned but unidentified
		expect(k162End(conn('K162', 'K162'))).toBe('a'); // degenerate: first match wins
	});
});

describe('scan progression helpers (site_type / name only — strength dropped)', () => {
	const scan = (site_type: string | null, name: string | null): ScanResult => ({
		sig_id: 'ABC-123',
		group: 'Cosmic Signature',
		site_type,
		name,
		wh_type: null,
		created_at: '2026-06-18T00:00:00.000Z',
		created_by: 1,
		updated_at: '2026-06-18T00:00:00.000Z',
		updated_by: 1
	});

	it('is unknown when site_type is null', () => {
		const r = scan(null, null);
		expect(scanIsUnknown(r)).toBe(true);
		expect(scanIsPartial(r)).toBe(false);
		expect(scanIsResolved(r)).toBe(false);
	});

	it('is partial when classified but unnamed', () => {
		const r = scan('Gas Site', null);
		expect(scanIsUnknown(r)).toBe(false);
		expect(scanIsPartial(r)).toBe(true);
		expect(scanIsResolved(r)).toBe(false);
	});

	it('is resolved once named', () => {
		const r = scan('Data Site', 'Unsecured Perimeter Information Center');
		expect(scanIsUnknown(r)).toBe(false);
		expect(scanIsPartial(r)).toBe(false);
		expect(scanIsResolved(r)).toBe(true);
	});
});
