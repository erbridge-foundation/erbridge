import { describe, it, expect } from 'vitest';
import { k162End } from './types';
import type { Connection, Signature } from './types';

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
