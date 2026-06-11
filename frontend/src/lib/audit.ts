// Pure, unit-testable helpers for the audit browser: the static event-type
// catalogue (mirrors backend/src/audit/mod.rs AuditEvent::event_type — kept in
// lockstep), the target-type catalogue, security-relevant-event detection (for
// browse-mode styling), and day-grouping of newest-first entries.

import type { AuditLogEntryDto } from '$lib/api';

/**
 * The 31 `AuditEvent` variants' wire strings. MUST stay in sync with the
 * backend catalogue (`AuditEvent::event_type`); the catalogue is stable —
 * existing strings are never renamed once shipped.
 */
export const EVENT_TYPES: readonly string[] = [
	'account_registered',
	'account_deletion_requested',
	'account_reactivated',
	'account_purged',
	'character_added',
	'character_removed',
	'character_set_main',
	'orphan_character_claimed',
	'api_key_created',
	'api_key_revoked',
	'server_admin_granted',
	'server_admin_revoked',
	'eve_character_blocked',
	'eve_character_unblocked',
	'blocked_login_rejected',
	'character_owner_mismatch',
	'map_created',
	'map_deleted',
	'acl_created',
	'acl_renamed',
	'acl_deleted',
	'acl_member_added',
	'acl_member_permission_changed',
	'acl_member_removed',
	'acl_attached_to_map',
	'acl_detached_from_map',
	'admin_map_ownership_changed',
	'admin_map_hard_deleted',
	'admin_acl_ownership_changed',
	'admin_acl_hard_deleted'
] as const;

/** The target-type catalogue for the closed-set select. */
export const TARGET_TYPES: readonly string[] = ['account', 'character', 'map', 'acl'] as const;

/**
 * Whether an event type is security-relevant and should be visually
 * distinguished while browsing: rejected logins, any hard-delete, character
 * blocks / owner-transfer detection, and any admin-override action.
 */
export function isSecurityEvent(eventType: string): boolean {
	return (
		eventType === 'blocked_login_rejected' ||
		eventType === 'eve_character_blocked' ||
		eventType === 'character_owner_mismatch' ||
		eventType.endsWith('_hard_deleted') ||
		eventType.startsWith('admin_') ||
		eventType.startsWith('server_admin_')
	);
}

export type DayGroupKey = 'today' | 'yesterday' | string;

export interface DayGroup {
	/** 'today' / 'yesterday' for relative headers, else an ISO date (YYYY-MM-DD). */
	key: DayGroupKey;
	entries: AuditLogEntryDto[];
}

/** Local-time midnight for a date, as ms since epoch. */
function startOfDay(d: Date): number {
	return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/**
 * Groups newest-first entries under day buckets, preserving order. The bucket
 * key is 'today' / 'yesterday' relative to `now` (default: current time), else
 * the entry day's ISO date. Grouping is by local calendar day.
 */
export function groupByDay(entries: AuditLogEntryDto[], now: Date = new Date()): DayGroup[] {
	const todayStart = startOfDay(now);
	const dayMs = 86_400_000;

	const groups: DayGroup[] = [];
	let current: DayGroup | null = null;

	for (const entry of entries) {
		const occurred = new Date(entry.occurred_at);
		const dayStart = startOfDay(occurred);

		let key: DayGroupKey;
		if (dayStart === todayStart) {
			key = 'today';
		} else if (dayStart === todayStart - dayMs) {
			key = 'yesterday';
		} else {
			// Local ISO date (YYYY-MM-DD) without UTC shift.
			const y = occurred.getFullYear();
			const m = String(occurred.getMonth() + 1).padStart(2, '0');
			const day = String(occurred.getDate()).padStart(2, '0');
			key = `${y}-${m}-${day}`;
		}

		if (!current || current.key !== key) {
			current = { key, entries: [] };
			groups.push(current);
		}
		current.entries.push(entry);
	}

	return groups;
}

/** The tiered time-window options for the window select, widest-last. */
export const WINDOW_TIERS: readonly string[] = ['7d', '30d', '90d', '365d'] as const;

/** The next wider window tier, or null at the widest. Drives the widen affordance. */
export function nextWiderWindow(window: string): string | null {
	const i = WINDOW_TIERS.indexOf(window);
	if (i === -1 || i === WINDOW_TIERS.length - 1) return null;
	return WINDOW_TIERS[i + 1];
}
