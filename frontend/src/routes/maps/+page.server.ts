import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listMaps, createMap, deleteMap } from '$lib/api';
import { failFrom } from '$lib/form-errors';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const maps = await listMaps(fetch, backend_internal_url(), cookie);
	return { maps };
};

function trimmed(data: FormData, key: string): string {
	const v = data.get(key);
	return typeof v === 'string' ? v.trim() : '';
}

function optional(data: FormData, key: string): string | null {
	const v = trimmed(data, key);
	return v === '' ? null : v;
}

export const actions: Actions = {
	create: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const backend = backend_internal_url();
		const data = await request.formData();
		const name = trimmed(data, 'name');
		const slug = trimmed(data, 'slug');
		const withDefaultAcl = data.get('default_acl') === 'on';
		if (name === '' || slug === '') {
			return fail(400, { action: 'create', code: 'bad_request', message: 'Name and slug are required' });
		}

		try {
			// `default_acl` asks the backend to mint a fresh ACL named after the map,
			// seed the account's main as an explicit admin (when a main exists; the
			// owner keeps implicit admin otherwise), attach it, and create the map —
			// all in one transaction. A failed map create (e.g. slug conflict) rolls
			// the ACL back too, so no orphan ACL can leak.
			await createMap(
				fetch,
				backend,
				{
					name,
					slug,
					description: optional(data, 'description'),
					default_acl: withDefaultAcl
				},
				cookie
			);
		} catch (e) {
			return failFrom('create', e);
		}
	},

	delete: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const id = trimmed(data, 'id');
		if (id === '') {
			return fail(400, { action: 'delete', code: 'bad_request', message: 'No map selected' });
		}

		try {
			await deleteMap(fetch, backend_internal_url(), id, cookie);
		} catch (e) {
			return failFrom('delete', e, { id });
		}
	}
};
