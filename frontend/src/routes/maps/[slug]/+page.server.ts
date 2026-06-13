import { error } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMapBySlug, ApiError } from '$lib/api';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, request, params }) => {
	const cookie = request.headers.get('cookie') ?? '';

	// Resolve the map by slug directly. The endpoint 404s on an unknown slug, a
	// soft-deleted map, or one the account cannot read. This route shows the map
	// canvas; settings/ACLs live under /maps/[slug]/settings.
	try {
		const map = await getMapBySlug(fetch, backend_internal_url(), params.slug, cookie);
		return { map };
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			error(404, 'Map not found');
		}
		throw e;
	}
};
