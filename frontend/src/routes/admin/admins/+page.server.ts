import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import {
	listAdminAccounts,
	searchCharacters,
	grantAdmin,
	revokeAdmin,
	ApiError,
	type AdminAccountDto
} from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const accounts = await listAdminAccounts(fetch, backend_internal_url(), cookie);
	const admins = accounts.filter((a: AdminAccountDto) => a.is_server_admin);
	return { admins };
};

export const actions: Actions = {
	search: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const q = data.get('q');
		if (typeof q !== 'string' || q.trim() === '') {
			return fail(400, { action: 'search', code: 'bad_request', message: 'A search term is required' });
		}

		try {
			const results = await searchCharacters(fetch, backend_internal_url(), q.trim(), cookie);
			return { action: 'search', query: q.trim(), results };
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'search', code: e.code, message: e.message });
			}
			return fail(500, { action: 'search', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	grant: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const accountId = data.get('account_id');
		if (typeof accountId !== 'string' || accountId === '') {
			return fail(400, { action: 'grant', code: 'bad_request', message: 'Missing account_id' });
		}

		try {
			await grantAdmin(fetch, backend_internal_url(), accountId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'grant', code: e.code, message: e.message });
			}
			return fail(500, { action: 'grant', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	revoke: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const accountId = data.get('account_id');
		if (typeof accountId !== 'string' || accountId === '') {
			return fail(400, { action: 'revoke', code: 'bad_request', message: 'Missing account_id' });
		}

		try {
			await revokeAdmin(fetch, backend_internal_url(), accountId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'revoke', code: e.code, message: e.message, accountId });
			}
			return fail(500, {
				action: 'revoke',
				code: 'internal_error',
				message: 'An unexpected error occurred',
				accountId
			});
		}
	}
};
