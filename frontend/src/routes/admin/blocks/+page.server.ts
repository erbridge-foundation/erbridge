import { fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { listBlocks, blockCharacter, unblockCharacter, ApiError } from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const blocks = await listBlocks(fetch, backend_internal_url(), cookie);
	return { blocks };
};

export const actions: Actions = {
	block: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const idRaw = data.get('eve_character_id');
		const reasonRaw = data.get('reason');

		const eve_character_id =
			typeof idRaw === 'string' ? Number(idRaw.trim()) : NaN;
		if (!Number.isInteger(eve_character_id) || eve_character_id <= 0) {
			return fail(400, { action: 'block', code: 'bad_request', message: 'A valid EVE character ID is required' });
		}
		const reason = typeof reasonRaw === 'string' && reasonRaw.trim() !== '' ? reasonRaw.trim() : null;

		try {
			await blockCharacter(fetch, backend_internal_url(), { eve_character_id, reason }, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'block', code: e.code, message: e.message });
			}
			return fail(500, { action: 'block', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	unblock: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const idRaw = data.get('eve_character_id');
		const eve_character_id = typeof idRaw === 'string' ? Number(idRaw) : NaN;
		if (!Number.isInteger(eve_character_id)) {
			return fail(400, { action: 'unblock', code: 'bad_request', message: 'Missing eve_character_id' });
		}

		try {
			await unblockCharacter(fetch, backend_internal_url(), eve_character_id, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, {
					action: 'unblock',
					code: e.code,
					message: e.message,
					eveCharacterId: eve_character_id
				});
			}
			return fail(500, {
				action: 'unblock',
				code: 'internal_error',
				message: 'An unexpected error occurred',
				eveCharacterId: eve_character_id
			});
		}
	}
};
