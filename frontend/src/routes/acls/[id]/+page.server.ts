import { error, fail } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import {
	listAcls,
	listAclMembers,
	addAclMember,
	updateAclMember,
	removeAclMember,
	searchEntities,
	ApiError,
	type AddMemberRequest
} from '$lib/api';
import type { PageServerLoad, Actions } from './$types';

const MIN_SEARCH_LEN = 3;

export const load: PageServerLoad = async ({ fetch, request, params }) => {
	const cookie = request.headers.get('cookie') ?? '';
	const backend = backend_internal_url();

	// No single GET /acls/{id}; resolve the ACL from the manageable list so the
	// detail can show its name and 404 on an ACL the account can't manage.
	const acls = await listAcls(fetch, backend, cookie);
	const acl = acls.find((a) => a.id === params.id);
	if (!acl) {
		error(404, 'ACL not found');
	}

	const members = await listAclMembers(fetch, backend, params.id, cookie);
	return { acl, members };
};

export const actions: Actions = {
	// Entity-search picker. Enforces the 3-char minimum, returns grouped results
	// + the `unavailable` flag (distinct from "no matches").
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

		// Map the picker's scope radio to the backend `categories` param so the ESI
		// search is narrowed (and quicker). 'any' (or anything unrecognized) searches
		// all three — leave categories undefined so the backend applies its default.
		const scope = data.get('scope');
		const categories =
			scope === 'character' || scope === 'corporation' || scope === 'alliance'
				? scope
				: undefined;

		try {
			const page = await searchEntities(fetch, backend_internal_url(), q.trim(), cookie, categories);
			return {
				action: 'search' as const,
				query: q.trim(),
				characters: page.characters,
				corporations: page.corporations,
				alliances: page.alliances,
				unavailable: page.unavailable
			};
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'search', code: e.code, message: e.message });
			}
			return fail(500, { action: 'search', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	// Add a member. The picker submits the already-resolved identity. Every
	// member carries `eve_entity_id` — the durable EVE id (character/corp/
	// alliance) — so the audit snapshot is uniform. A character additionally
	// carries `character_id` (the eve_character.id UUID, the internal FK link).
	addMember: async ({ request, fetch, params }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const memberType = data.get('member_type');
		const permission = data.get('permission');
		const name = data.get('name');

		if (
			typeof memberType !== 'string' ||
			!['character', 'corporation', 'alliance'].includes(memberType) ||
			typeof permission !== 'string' ||
			permission === ''
		) {
			return fail(400, { action: 'addMember', code: 'bad_request', message: 'Select a member and a permission' });
		}

		const body: AddMemberRequest = {
			member_type: memberType,
			permission,
			name: typeof name === 'string' ? name : ''
		};

		// The durable EVE id is required for every member type.
		const idRaw = data.get('eve_entity_id');
		const eve_entity_id = typeof idRaw === 'string' ? Number(idRaw) : NaN;
		if (!Number.isInteger(eve_entity_id) || eve_entity_id <= 0) {
			return fail(400, { action: 'addMember', code: 'bad_request', message: 'No entity selected' });
		}
		body.eve_entity_id = eve_entity_id;

		if (memberType === 'character') {
			const characterId = data.get('character_id');
			if (typeof characterId !== 'string' || characterId === '') {
				return fail(400, { action: 'addMember', code: 'bad_request', message: 'No character selected' });
			}
			body.character_id = characterId;
		}

		try {
			await addAclMember(fetch, backend_internal_url(), params.id, body, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'addMember', code: e.code, message: e.message });
			}
			return fail(500, { action: 'addMember', code: 'internal_error', message: 'An unexpected error occurred' });
		}
	},

	updateMember: async ({ request, fetch, params }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const memberId = data.get('member_id');
		const permission = data.get('permission');
		if (typeof memberId !== 'string' || memberId === '' || typeof permission !== 'string' || permission === '') {
			return fail(400, {
				action: 'updateMember',
				code: 'bad_request',
				message: 'Select a permission',
				memberId: typeof memberId === 'string' ? memberId : undefined
			});
		}

		try {
			await updateAclMember(fetch, backend_internal_url(), params.id, memberId, { permission }, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'updateMember', code: e.code, message: e.message, memberId });
			}
			return fail(500, { action: 'updateMember', code: 'internal_error', message: 'An unexpected error occurred', memberId });
		}
	},

	removeMember: async ({ request, fetch, params }) => {
		const cookie = request.headers.get('cookie') ?? '';
		const data = await request.formData();
		const memberId = data.get('member_id');
		if (typeof memberId !== 'string' || memberId === '') {
			return fail(400, { action: 'removeMember', code: 'bad_request', message: 'No member selected' });
		}

		try {
			await removeAclMember(fetch, backend_internal_url(), params.id, memberId, cookie);
		} catch (e) {
			if (e instanceof ApiError) {
				return fail(e.status, { action: 'removeMember', code: e.code, message: e.message, memberId });
			}
			return fail(500, { action: 'removeMember', code: 'internal_error', message: 'An unexpected error occurred', memberId });
		}
	}
};
