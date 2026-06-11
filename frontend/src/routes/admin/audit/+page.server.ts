import { backend_internal_url } from '$lib/server/env';
import { listAuditLog, type AuditLogQuery } from '$lib/api';
import type { PageServerLoad } from './$types';

const PAGE_LIMIT = 50;
const DEFAULT_WINDOW = '7d';

/** The active filter state echoed back to the page for chips, selects, and
 * the "load older" / widen links. `window` always has a value (defaulting to
 * the 7-day browse window) so the select renders a current selection. */
export interface AuditFilters {
	event_type: string;
	actor: string;
	target_type: string;
	target_id: string;
	q: string;
	window: string;
}

export const load: PageServerLoad = async ({ fetch, request, url }) => {
	const cookie = request.headers.get('cookie') ?? '';

	const filters: AuditFilters = {
		event_type: url.searchParams.get('event_type') ?? '',
		actor: url.searchParams.get('actor') ?? '',
		target_type: url.searchParams.get('target_type') ?? '',
		target_id: url.searchParams.get('target_id') ?? '',
		q: url.searchParams.get('q') ?? '',
		window: url.searchParams.get('window') || DEFAULT_WINDOW
	};

	const query: AuditLogQuery = { limit: PAGE_LIMIT, window: filters.window };
	if (filters.event_type) query.event_type = filters.event_type;
	if (filters.actor) query.actor = filters.actor;
	if (filters.target_type) query.target_type = filters.target_type;
	if (filters.target_id) query.target_id = filters.target_id;
	if (filters.q) query.q = filters.q;
	const before = url.searchParams.get('before');
	if (before) query.before = before;

	const page = await listAuditLog(fetch, backend_internal_url(), query, cookie);
	return { page, filters, pageLimit: PAGE_LIMIT };
};
