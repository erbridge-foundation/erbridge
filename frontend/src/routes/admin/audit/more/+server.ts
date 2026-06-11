// Browser → SvelteKit → backend proxy for audit infinite-scroll pagination.
//
// The initial page comes from /admin/audit's +page.server.ts `load`. Subsequent
// older pages (fetched as the admin scrolls within the active window) come
// through here: the browser cannot reach the internal backend URL directly, so
// this endpoint forwards the session cookie and the same filter axes plus the
// `before` keyset cursor. It returns the raw `{ data: AuditLogPageDto }`
// envelope. Lives at a child path so it does not collide with the page's own
// GET route.

import { json, error } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listAuditLog, ApiError, type AuditLogQuery } from '$lib/api';
import type { RequestHandler } from './$types';

const PAGE_LIMIT = 50;

export const GET: RequestHandler = async ({ fetch, request, url }) => {
	const cookie = request.headers.get('cookie') ?? '';

	const query: AuditLogQuery = { limit: PAGE_LIMIT };
	const window = url.searchParams.get('window');
	const event_type = url.searchParams.get('event_type');
	const actor = url.searchParams.get('actor');
	const target_type = url.searchParams.get('target_type');
	const target_id = url.searchParams.get('target_id');
	const q = url.searchParams.get('q');
	const before = url.searchParams.get('before');
	if (window) query.window = window;
	if (event_type) query.event_type = event_type;
	if (actor) query.actor = actor;
	if (target_type) query.target_type = target_type;
	if (target_id) query.target_id = target_id;
	if (q) query.q = q;
	if (before) query.before = before;

	try {
		const page = await listAuditLog(fetch, backend_internal_url(), query, cookie);
		return json({ data: page });
	} catch (e) {
		if (e instanceof ApiError) error(e.status, e.message);
		error(500, 'Failed to load audit log');
	}
};
