import { backend_internal_url } from '$lib/server/env';
import { listAuditLog, type AuditLogQuery } from '$lib/api';
import type { PageServerLoad } from './$types';

const PAGE_LIMIT = 50;

export const load: PageServerLoad = async ({ fetch, request, url }) => {
	const cookie = request.headers.get('cookie') ?? '';

	const filters = {
		event_type: url.searchParams.get('event_type') ?? '',
		actor: url.searchParams.get('actor') ?? '',
		target_type: url.searchParams.get('target_type') ?? '',
		target_id: url.searchParams.get('target_id') ?? '',
		target_name: url.searchParams.get('target_name') ?? ''
	};

	const query: AuditLogQuery = { limit: PAGE_LIMIT };
	if (filters.event_type) query.event_type = filters.event_type;
	if (filters.actor) query.actor = filters.actor;
	if (filters.target_type) query.target_type = filters.target_type;
	if (filters.target_id) query.target_id = filters.target_id;
	if (filters.target_name) query.target_name = filters.target_name;
	const before = url.searchParams.get('before');
	if (before) query.before = before;

	const page = await listAuditLog(fetch, backend_internal_url(), query, cookie);
	return { page, filters };
};
