import { error } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listMaps } from '$lib/api';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, request, params }) => {
	const cookie = request.headers.get('cookie') ?? '';

	// The slug is resolved from the account's maps list (no backend slug-lookup
	// endpoint). This route shows the map canvas; settings/ACLs live under
	// /maps/[slug]/settings.
	const maps = await listMaps(fetch, backend_internal_url(), cookie);
	const map = maps.find((m) => m.slug === params.slug);
	if (!map) {
		error(404, 'Map not found');
	}

	return { map };
};
