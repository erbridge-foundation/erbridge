import { describe, it, expect } from 'vitest';
import { relativeTime, localAndEveTime } from './relative-time';

const now = new Date('2026-06-18T12:00:00.000Z');
const ago = (ms: number) => new Date(now.getTime() - ms).toISOString();

describe('relativeTime', () => {
	it('shows "just now" under a minute', () => {
		expect(relativeTime(ago(0), now)).toBe('just now');
		expect(relativeTime(ago(59_000), now)).toBe('just now');
	});

	it('shows minutes under an hour', () => {
		expect(relativeTime(ago(60_000), now)).toBe('1m');
		expect(relativeTime(ago(5 * 60_000), now)).toBe('5m');
		expect(relativeTime(ago(59 * 60_000), now)).toBe('59m');
	});

	it('shows hours under a day', () => {
		expect(relativeTime(ago(60 * 60_000), now)).toBe('1h');
		expect(relativeTime(ago(23 * 60 * 60_000), now)).toBe('23h');
	});

	it('shows days beyond that', () => {
		expect(relativeTime(ago(24 * 60 * 60_000), now)).toBe('1d');
		expect(relativeTime(ago(3 * 24 * 60 * 60_000), now)).toBe('3d');
	});

	it('clamps future timestamps (clock skew) to "just now"', () => {
		expect(relativeTime(new Date(now.getTime() + 5_000).toISOString(), now)).toBe('just now');
	});

	it('returns an em-dash for an unparseable value', () => {
		expect(relativeTime('not-a-date', now)).toBe('—');
	});
});

describe('localAndEveTime', () => {
	// EVE time = UTC (what we store), so it's a pure format with no conversion. The
	// test pins to the value we know regardless of the runner's locale/timezone.
	it('shows EVE (UTC) time labelled, 24-hour, for the stored instant', () => {
		const out = localAndEveTime('2026-06-18T20:50:00.000Z');
		// The EVE half is deterministic: UTC, 24h → "20:50", labelled EVE.
		expect(out).toMatch(/20:50.*EVE$/);
		expect(out).toContain('2026');
		// Two halves separated by the middot.
		expect(out).toContain('·');
	});

	it('returns an em-dash for an unparseable value', () => {
		expect(localAndEveTime('nope')).toBe('—');
	});
});
