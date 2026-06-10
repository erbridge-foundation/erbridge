import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import {
	listMaps,
	createMap,
	deleteMap,
	createAcl,
	addAclMember,
	getMe,
	ApiError
} from '$lib/api';
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
			let aclId: string | undefined;

			if (withDefaultAcl) {
				// Create a reusable ACL named after the map, then seed it with the
				// account's main character as an explicit admin. If the account has
				// no main, the ACL is created empty (the owner still has implicit
				// admin via the resolver). The map is then attached to this ACL.
				const acl = await createAcl(fetch, backend, name, cookie);
				aclId = acl.id;

				try {
					const me = await getMe(fetch, backend, cookie);
					const main = me.characters.find((c) => c.is_main);
					if (main) {
						await addAclMember(
							fetch,
							backend,
							acl.id,
							{
								member_type: 'character',
								character_id: main.id,
								name: main.name,
								permission: 'admin'
							},
							cookie
						);
					}
				} catch {
					// Seeding the owner member is best-effort: a failure here leaves
					// the ACL empty (owner keeps implicit admin) rather than aborting
					// the whole map creation.
				}
			}

			await createMap(
				fetch,
				backend,
				{ name, slug, description: optional(data, 'description'), acl_id: aclId },
				cookie
			);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'create', code: e.code, message: e.message });
			}
			return fail(500, { action: 'create', code: 'internal_error', message: 'An unexpected error occurred' });
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
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'delete', code: e.code, message: e.message, id });
			}
			return fail(500, { action: 'delete', code: 'internal_error', message: 'An unexpected error occurred', id });
		}
	}
};
