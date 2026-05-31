import { fail } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { backend_internal_url } from '$lib/server/env';
import {
	listBlocks,
	blockCharacter,
	unblockCharacter,
	searchCharacters,
	searchCharactersEsi,
	ApiError,
	type CharacterSearchResultDto,
	type EsiCharacterSearchResultDto
} from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

const MIN_SEARCH_LEN = 3;
// Public ESI base for the best-effort corp lookup. Overridable (ESI_PUBLIC_BASE)
// so e2e can point it at the mock backend instead of reaching real ESI.
const ESI_BASE = env.ESI_PUBLIC_BASE || 'https://esi.evetech.net/latest';

export const load: PageServerLoad = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const blocks = await listBlocks(fetch, backend_internal_url(), cookie);
	return { blocks };
};

export const actions: Actions = {
	// Local-DB name search (the picker's first source). Returns the enriched
	// result shape (portrait + already_blocked).
	search: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const q = data.get('q');
		if (typeof q !== 'string' || q.trim().length < MIN_SEARCH_LEN) {
			return fail(400, {
				action: 'search',
				code: 'too_short',
				message: `Type at least ${MIN_SEARCH_LEN} characters`
			});
		}

		try {
			const results = await searchCharacters(fetch, backend_internal_url(), q.trim(), cookie);
			return { action: 'search', query: q.trim(), results } satisfies {
				action: 'search';
				query: string;
				results: CharacterSearchResultDto[];
			};
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'search', code: e.code, message: e.message });
			}
			return fail(500, { action: 'search', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	// ESI fallback search (opt-in when the local search comes up empty).
	esiSearch: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const q = data.get('q');
		if (typeof q !== 'string' || q.trim().length < MIN_SEARCH_LEN) {
			return fail(400, {
				action: 'esiSearch',
				code: 'too_short',
				message: `Type at least ${MIN_SEARCH_LEN} characters`
			});
		}

		try {
			const page = await searchCharactersEsi(fetch, backend_internal_url(), q.trim(), cookie);
			return {
				action: 'esiSearch',
				query: q.trim(),
				results: page.results,
				unavailable: page.unavailable
			} satisfies {
				action: 'esiSearch';
				query: string;
				results: EsiCharacterSearchResultDto[];
				unavailable: boolean;
			};
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'esiSearch', code: e.code, message: e.message });
			}
			return fail(500, { action: 'esiSearch', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	// Corp lookup for the confirmation dialog (public ESI, no auth). Best-effort:
	// a failure returns a null corp so the confirm still works.
	corpLookup: async ({ request, fetch }) => {
		const data = await request.formData();
		const idRaw = data.get('eve_character_id');
		const eve_character_id = typeof idRaw === 'string' ? Number(idRaw) : NaN;
		if (!Number.isInteger(eve_character_id) || eve_character_id <= 0) {
			return fail(400, { action: 'corpLookup', code: 'bad_request', message: 'Missing eve_character_id' });
		}

		let corporation_name: string | null = null;
		try {
			const charRes = await fetch(`${ESI_BASE}/characters/${eve_character_id}/`);
			if (charRes.ok) {
				const char = (await charRes.json()) as { corporation_id?: number };
				if (char.corporation_id) {
					const corpRes = await fetch(`${ESI_BASE}/corporations/${char.corporation_id}/`);
					if (corpRes.ok) {
						const corp = (await corpRes.json()) as { name?: string };
						corporation_name = corp.name ?? null;
					}
				}
			}
		} catch {
			corporation_name = null;
		}

		return { action: 'corpLookup', eve_character_id, corporation_name };
	},

	// Block a character chosen from the picker. The id is resolved by the picker,
	// not free-typed.
	block: async ({ request, fetch }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const idRaw = data.get('eve_character_id');
		const reasonRaw = data.get('reason');

		const eve_character_id = typeof idRaw === 'string' ? Number(idRaw) : NaN;
		if (!Number.isInteger(eve_character_id) || eve_character_id <= 0) {
			return fail(400, { action: 'block', code: 'bad_request', message: 'No character selected' });
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
