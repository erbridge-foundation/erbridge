import { backend_internal_url } from '$lib/server/env';
import { listAdminAccounts } from '$lib/api';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	// The accounts list carries every account with its characters and each
	// character's token_status — the whole grid renders from this, with no
	// per-account round-trip and no character search.
	const accounts = await listAdminAccounts(fetch, backend_internal_url(), cookie);
	return { accounts };
};
