import { error } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMe } from '$lib/api';
import type { LayoutServerLoad } from './$types';

// Server-side gate for the entire /admin route group. A non-admin (or
// unauthenticated) caller gets a 404 — the existence of the admin pages is not
// disclosed (per the server-administration spec). The cookie is forwarded to
// the backend per the project's load-fetch pattern.
export const load: LayoutServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';

	let isAdmin = false;
	try {
		const me = await getMe(fetch, backend_internal_url(), cookie);
		isAdmin = me.account.is_server_admin;
	} catch {
		// Any failure (401, backend down, …) is treated as "not an admin": we do
		// not disclose the admin section to anyone we cannot positively confirm.
		isAdmin = false;
	}

	if (!isAdmin) {
		error(404, 'Not found');
	}

	return {};
};
