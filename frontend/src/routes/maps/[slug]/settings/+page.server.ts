import { error, fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMapBySlug, listAcls, updateMap, attachAcl, detachAcl, ApiError } from '$lib/api';
import { failFrom } from '$lib/form-errors';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request, params }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const backend = backend_internal_url();

	// Resolve the map by slug directly (404 on unknown/soft-deleted/unreadable).
	let map;
	try {
		map = await getMapBySlug(fetch, backend, params.slug, cookie);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			error(404, 'Map not found');
		}
		throw e;
	}

	// The account's manageable ACLs feed the attach control; only those not
	// already attached are offered.
	const acls = await listAcls(fetch, backend, cookie);
	const attachedIds = new Set(map.acls.map((a) => a.id));
	const attachable = acls.filter((a) => !attachedIds.has(a.id));

	return { map, attachable };
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
	edit: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const id = trimmed(data, 'id');
		const name = trimmed(data, 'name');
		const slug = trimmed(data, 'slug');
		if (id === '' || name === '' || slug === '') {
			return fail(400, { action: 'edit', code: 'bad_request', message: 'Name and slug are required' });
		}

		try {
			await updateMap(
				fetch,
				backend_internal_url(),
				id,
				{ name, slug, description: optional(data, 'description') },
				cookie
			);
		} catch (e) {
			return failFrom('edit', e);
		}
	},

	attach: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const mapId = data.get('map_id');
		const aclId = data.get('acl_id');
		if (typeof mapId !== 'string' || typeof aclId !== 'string' || mapId === '' || aclId === '') {
			return fail(400, { action: 'attach', code: 'bad_request', message: 'Select an ACL to attach' });
		}

		try {
			await attachAcl(fetch, backend_internal_url(), mapId, aclId, cookie);
		} catch (e) {
			return failFrom('attach', e);
		}
	},

	detach: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const mapId = data.get('map_id');
		const aclId = data.get('acl_id');
		if (typeof mapId !== 'string' || typeof aclId !== 'string' || mapId === '' || aclId === '') {
			return fail(400, {
				action: 'detach',
				code: 'bad_request',
				message: 'No ACL selected',
				aclId: typeof aclId === 'string' ? aclId : undefined
			});
		}

		try {
			await detachAcl(fetch, backend_internal_url(), mapId, aclId, cookie);
		} catch (e) {
			return failFrom('detach', e, { aclId });
		}
	}
};
