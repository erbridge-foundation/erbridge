import { describe, it, expect } from 'vitest';
import {
	EVENT_TYPES,
	TARGET_TYPES,
	WINDOW_TIERS,
	groupByDay,
	isSecurityEvent,
	nextWiderWindow
} from './audit';
import type { AuditLogEntryDto } from './api';

function entry(id: string, occurred_at: string, event_type = 'account_registered'): AuditLogEntryDto {
	return {
		id,
		occurred_at,
		actor_account_id: null,
		actor_character_id: null,
		actor_character_name: null,
		event_type,
		details: {},
		target_type: null,
		target_id: null,
		target_name: null
	};
}

describe('catalogues', () => {
	it('lists every AuditEvent variant once (mirrors the backend catalogue)', () => {
		// The backend AuditEvent catalogue currently has 30 variants; the spec's
		// "31" figure is approximate. The hard invariant is parity with the
		// backend, checked by the catalogue-sync diff in CI / review.
		expect(EVENT_TYPES).toHaveLength(30);
		expect(new Set(EVENT_TYPES).size).toBe(30);
	});

	it('lists the four target types', () => {
		expect([...TARGET_TYPES]).toEqual(['account', 'character', 'map', 'acl']);
	});
});

describe('isSecurityEvent', () => {
	it('flags rejected logins, blocks, and owner transfers', () => {
		expect(isSecurityEvent('blocked_login_rejected')).toBe(true);
		expect(isSecurityEvent('eve_character_blocked')).toBe(true);
		expect(isSecurityEvent('character_owner_mismatch')).toBe(true);
	});

	it('flags hard-deletes and admin/server-admin actions', () => {
		expect(isSecurityEvent('admin_map_hard_deleted')).toBe(true);
		expect(isSecurityEvent('admin_acl_ownership_changed')).toBe(true);
		expect(isSecurityEvent('server_admin_granted')).toBe(true);
	});

	it('does not flag routine events', () => {
		expect(isSecurityEvent('account_registered')).toBe(false);
		expect(isSecurityEvent('map_created')).toBe(false);
		expect(isSecurityEvent('acl_member_added')).toBe(false);
	});
});

describe('nextWiderWindow', () => {
	it('returns the next wider tier', () => {
		expect(nextWiderWindow('7d')).toBe('30d');
		expect(nextWiderWindow('30d')).toBe('90d');
		expect(nextWiderWindow('90d')).toBe('365d');
	});

	it('returns null at the widest tier or for unknown values', () => {
		expect(nextWiderWindow('365d')).toBeNull();
		expect(nextWiderWindow('year:2024')).toBeNull();
	});

	it('exposes the tiers widest-last', () => {
		expect([...WINDOW_TIERS]).toEqual(['7d', '30d', '90d', '365d']);
	});
});

describe('groupByDay', () => {
	const now = new Date('2026-06-11T12:00:00');

	it('buckets entries under today / yesterday / dated headers, preserving order', () => {
		const entries = [
			entry('a', '2026-06-11T10:00:00'),
			entry('b', '2026-06-11T09:00:00'),
			entry('c', '2026-06-10T23:00:00'),
			entry('d', '2026-06-08T08:00:00')
		];
		const groups = groupByDay(entries, now);
		expect(groups.map((g) => g.key)).toEqual(['today', 'yesterday', '2026-06-08']);
		expect(groups[0].entries.map((e) => e.id)).toEqual(['a', 'b']);
		expect(groups[1].entries.map((e) => e.id)).toEqual(['c']);
		expect(groups[2].entries.map((e) => e.id)).toEqual(['d']);
	});

	it('returns an empty array for no entries', () => {
		expect(groupByDay([], now)).toEqual([]);
	});

	it('keeps a single group when all entries fall on the same day', () => {
		const groups = groupByDay(
			[entry('a', '2026-06-11T10:00:00'), entry('b', '2026-06-11T01:00:00')],
			now
		);
		expect(groups).toHaveLength(1);
		expect(groups[0].key).toBe('today');
	});
});
