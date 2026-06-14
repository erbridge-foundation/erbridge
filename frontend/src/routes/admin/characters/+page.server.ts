import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listAdminAccounts, hardDeletePreview, hardDeleteAccount, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	// The accounts list carries every account with its characters and each
	// character's token_status — the whole grid renders from this, with no
	// per-account round-trip and no character search.
	const accounts = await listAdminAccounts(fetch, backend_internal_url(), cookie);
	return { accounts };
};

export const actions: Actions = {
	// Fetch the blast-radius preview for an account before an irreversible
	// hard-delete. Read-only — the UI shows this and an explicit confirm before
	// dispatching the `delete` action.
	preview: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const accountId = data.get('account_id');
		if (typeof accountId !== 'string' || accountId === '') {
			return fail(400, { action: 'preview', code: 'bad_request', message: 'Missing account_id' });
		}

		try {
			const preview = await hardDeletePreview(fetch, backend_internal_url(), accountId, cookie);
			return { action: 'preview', accountId, preview };
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'preview', code: e.code, message: e.message, accountId });
			}
			return fail(500, {
				action: 'preview',
				code: 'internal_error',
				message: 'An unexpected error occurred',
				accountId
			});
		}
	},

	// Irreversible: hard-delete the account. Returns the blast radius removed.
	delete: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const accountId = data.get('account_id');
		if (typeof accountId !== 'string' || accountId === '') {
			return fail(400, { action: 'delete', code: 'bad_request', message: 'Missing account_id' });
		}

		try {
			const removed = await hardDeleteAccount(fetch, backend_internal_url(), accountId, cookie);
			return { action: 'delete', accountId, removed };
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'delete', code: e.code, message: e.message, accountId });
			}
			return fail(500, {
				action: 'delete',
				code: 'internal_error',
				message: 'An unexpected error occurred',
				accountId
			});
		}
	}
};
