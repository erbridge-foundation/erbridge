import { redirect, fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listKeys, createKey, deleteKey, deleteAccount, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const keys = await listKeys(fetch, backend_internal_url(), cookie);
	return { keys };
};

export const actions: Actions = {
	createKey: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const nameRaw = data.get('name');
		const expiresRaw = data.get('expires_at');

		if (typeof nameRaw !== 'string' || nameRaw.trim() === '') {
			return fail(400, { code: 'bad_request', message: 'Name is required' });
		}
		const expires_at =
			typeof expiresRaw === 'string' && expiresRaw !== '' ? expiresRaw : null;

		try {
			const created = await createKey(
				fetch,
				backend_internal_url(),
				{ name: nameRaw.trim(), expires_at },
				cookie
			);
			return { createdKey: created };
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { code: e.code, message: e.message });
			}
			return fail(500, { code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	revokeKey: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const keyId = data.get('key_id');
		if (typeof keyId !== 'string') {
			return fail(400, { code: 'bad_request', message: 'Missing key_id' });
		}

		try {
			await deleteKey(fetch, backend_internal_url(), keyId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { code: e.code, message: e.message, keyId });
			}
			return fail(500, { code: 'internal_error', message: 'An unexpected error occurred', keyId });
		}
	},

	deleteAccount: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		try {
			await deleteAccount(fetch, backend_internal_url(), cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { code: e.code, message: e.message });
			}
			return fail(500, { code: 'internal_error', message: 'An unexpected error occurred' });
		}
		redirect(303, '/login');
	}
};
