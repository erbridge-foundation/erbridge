import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { setMainCharacter, deleteCharacter, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ parent }) => {
	const { me } = await parent();
	return { characters: me?.characters ?? [] };
};

export const actions: Actions = {
	setMain: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const characterId = data.get('character_id');
		if (typeof characterId !== 'string') return fail(400, { code: 'bad_request', message: 'Missing character_id' });

		try {
			await setMainCharacter(fetch, backend_internal_url(), characterId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { code: e.code, message: e.message, characterId });
			}
			return fail(500, { code: 'internal_error', message: 'An unexpected error occurred', characterId });
		}
	},

	remove: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const characterId = data.get('character_id');
		if (typeof characterId !== 'string') return fail(400, { code: 'bad_request', message: 'Missing character_id' });

		try {
			await deleteCharacter(fetch, backend_internal_url(), characterId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { code: e.code, message: e.message, characterId });
			}
			return fail(500, { code: 'internal_error', message: 'An unexpected error occurred', characterId });
		}
	}
};
