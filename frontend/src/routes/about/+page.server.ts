import { backend_internal_url } from '$lib/server/env';
import { getHealth } from '$lib/api';
import type { PageServerLoad } from './$types';

// /api/health is public — no cookie forwarded. The page MUST render whether or
// not health is reachable, so any error resolves to { health: null } rather
// than throwing.
export const load: PageServerLoad = async ({ fetch }) => {
	try {
		const health = await getHealth(fetch, backend_internal_url());
		return { health, healthError: null };
	} catch (e) {
		const message = e instanceof Error ? e.message : 'Unknown error';
		return { health: null, healthError: { message } };
	}
};
