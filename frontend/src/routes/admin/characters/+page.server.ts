import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listAdminAccounts, searchCharacters, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	// The accounts list carries every account with its characters and each
	// character's token_status — enough to render the inspect dialog without a
	// per-account round-trip.
	const accounts = await listAdminAccounts(fetch, backend_internal_url(), cookie);
	return { accounts };
};

export const actions: Actions = {
	search: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const q = data.get('q');
		if (typeof q !== 'string' || q.trim() === '') {
			return fail(400, {
				action: 'search',
				code: 'bad_request',
				message: 'A search term is required'
			});
		}

		try {
			const results = await searchCharacters(fetch, backend_internal_url(), q.trim(), cookie);
			return { action: 'search', query: q.trim(), results };
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'search', code: e.code, message: e.message });
			}
			return fail(500, {
				action: 'search',
				code: 'internal_error',
				message: 'An unexpected error occurred'
			});
		}
	}
};
