/**
 * Compact relative-time formatting for the map sidebar's "Updated" column.
 *
 * Scans and structures carry ISO-8601 UTC timestamps (see TrackingMeta); the
 * sidebar shows how long ago a record was last updated as a terse token ("just
 * now", "5m", "2h", "3d") rather than a full datetime. The precise timestamps are
 * surfaced separately (a tooltip) — this is the at-a-glance recency cue only.
 *
 * Pure and deterministic given `now`, so it's unit-testable without faking the
 * clock. Future-dated inputs (clock skew) clamp to "just now".
 */
export function relativeTime(iso: string, now: Date = new Date()): string {
	const then = new Date(iso).getTime();
	if (Number.isNaN(then)) return '—';
	const deltaMs = now.getTime() - then;
	if (deltaMs < 60_000) return 'just now'; // also covers small future skew
	const mins = Math.floor(deltaMs / 60_000);
	if (mins < 60) return `${mins}m`;
	const hours = Math.floor(mins / 60);
	if (hours < 24) return `${hours}h`;
	const days = Math.floor(hours / 24);
	return `${days}d`;
}

// Locale-aware date+time in the BROWSER's locale + local timezone (passing
// `undefined` as the locale tells Intl to use the browser's preference).
const localFmt = new Intl.DateTimeFormat(undefined, {
	dateStyle: 'medium',
	timeStyle: 'short'
});
// The same instant in EVE time. EVE runs on UTC, which is exactly what we store, so
// this is a pure format (no conversion) — 24-hour, matching the in-game convention.
const eveFmt = new Intl.DateTimeFormat(undefined, {
	dateStyle: 'medium',
	timeStyle: 'short',
	hour12: false,
	timeZone: 'UTC'
});

/**
 * A human-readable absolute timestamp for the provenance tooltip, showing BOTH the
 * user's local time AND EVE time (UTC). e.g.
 *   "Jun 18, 2026, 8:50 PM · 18 Jun 2026, 20:50 EVE"
 * Local uses the browser's locale + timezone; EVE is the same instant in UTC.
 * Returns "—" for an unparseable input.
 */
export function localAndEveTime(iso: string): string {
	const d = new Date(iso);
	if (Number.isNaN(d.getTime())) return '—';
	return `${localFmt.format(d)} · ${eveFmt.format(d)} EVE`;
}
