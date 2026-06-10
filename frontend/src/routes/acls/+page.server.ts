import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listAcls, createAcl, renameAcl, deleteAcl, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const acls = await listAcls(fetch, backend_internal_url(), cookie);
	return { acls };
};

function trimmed(data: FormData, key: string): string {
	const v = data.get(key);
	return typeof v === 'string' ? v.trim() : '';
}

export const actions: Actions = {
	create: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const name = trimmed(data, 'name');
		if (name === '') {
			return fail(400, { action: 'create', code: 'bad_request', message: 'Name is required' });
		}

		try {
			await createAcl(fetch, backend_internal_url(), name, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'create', code: e.code, message: e.message });
			}
			return fail(500, { action: 'create', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	rename: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const id = trimmed(data, 'id');
		const name = trimmed(data, 'name');
		if (id === '' || name === '') {
			return fail(400, { action: 'rename', code: 'bad_request', message: 'Name is required', id });
		}

		try {
			await renameAcl(fetch, backend_internal_url(), id, name, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'rename', code: e.code, message: e.message, id });
			}
			return fail(500, { action: 'rename', code: 'internal_error', message: 'An unexpected error occurred', id });
		}
	},

	delete: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const id = trimmed(data, 'id');
		if (id === '') {
			return fail(400, { action: 'delete', code: 'bad_request', message: 'No ACL selected' });
		}

		try {
			await deleteAcl(fetch, backend_internal_url(), id, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'delete', code: e.code, message: e.message, id });
			}
			return fail(500, { action: 'delete', code: 'internal_error', message: 'An unexpected error occurred', id });
		}
	}
};
