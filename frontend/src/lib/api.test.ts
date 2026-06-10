import { describe, it, expect, vi } from 'vitest';
import {
	ApiError,
	getMe,
	getHealth,
	deleteCharacter,
	deleteAccount,
	setMainCharacter,
	listKeys,
	createKey,
	deleteKey,
	listMaps,
	createMap,
	updateMap,
	deleteMap,
	attachAcl,
	detachAcl,
	listAcls,
	createAcl,
	renameAcl,
	addAclMember,
	updateAclMember,
	removeAclMember,
	searchEntities
} from './api';
import type {
	MeResponse,
	CharacterDto,
	HealthResponse,
	KeyMetadataDto,
	CreatedKeyDto,
	MapDto,
	AclDto,
	AclMemberDto,
	EntitySearchPageDto
} from './api';

const BACKEND = 'http://backend:3000';
const COOKIE = 'session=abc.def.ghi';

function mockJsonFetch(status: number, body: unknown): typeof globalThis.fetch {
	return vi.fn(async () => {
		const response = new Response(body === undefined ? null : JSON.stringify(body), {
			status,
			headers: { 'content-type': 'application/json' }
		});
		return response;
	}) as unknown as typeof globalThis.fetch;
}

function mockNoContentFetch(): typeof globalThis.fetch {
	return vi.fn(async () => new Response(null, { status: 204 })) as unknown as typeof globalThis.fetch;
}

describe('api.request', () => {
	it('getMe returns unwrapped data on 200', async () => {
		const me: MeResponse = {
			account: { id: 'a', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: []
		};
		const fetch = mockJsonFetch(200, { data: me });
		const result = await getMe(fetch, BACKEND, COOKIE);
		expect(result).toEqual(me);
	});

	it('getMe forwards the cookie header', async () => {
		const fetch = mockJsonFetch(200, { data: { account: {}, characters: [] } });
		await getMe(fetch, BACKEND, COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/me`,
			expect.objectContaining({ headers: { cookie: COOKIE } })
		);
	});

	it('deleteCharacter returns undefined on 204 (no body)', async () => {
		const fetch = mockNoContentFetch();
		await expect(deleteCharacter(fetch, BACKEND, 'char-id', COOKIE)).resolves.toBeUndefined();
	});

	it('deleteAccount returns undefined on 204 (no body)', async () => {
		const fetch = mockNoContentFetch();
		await expect(deleteAccount(fetch, BACKEND, COOKIE)).resolves.toBeUndefined();
	});

	it('throws ApiError with envelope code/message on 4xx', async () => {
		const fetch = mockJsonFetch(409, {
			error: { code: 'cannot_remove_main', message: 'Cannot remove the main character' }
		});
		await expect(deleteCharacter(fetch, BACKEND, 'char-id', COOKIE)).rejects.toMatchObject({
			code: 'cannot_remove_main',
			message: 'Cannot remove the main character',
			status: 409
		});
	});

	it('throws ApiError with defaults when error body is non-JSON', async () => {
		const fetch = vi.fn(
			async () => new Response('plain text error', { status: 500 })
		) as unknown as typeof globalThis.fetch;
		await expect(getMe(fetch, BACKEND, COOKIE)).rejects.toBeInstanceOf(ApiError);
		await expect(getMe(fetch, BACKEND, COOKIE)).rejects.toMatchObject({ status: 500 });
	});

	it('setMainCharacter POSTs and returns the updated character on 200', async () => {
		const updated: CharacterDto = {
			id: 'c1',
			eve_character_id: 123,
			name: 'Pilot',
			corporation_id: 1,
			corporation_name: 'Corp',
			alliance_id: null,
			alliance_name: null,
			is_main: true,
			portrait_url: 'https://x',
			token_status: 'active'
		};
		const fetch = mockJsonFetch(200, { data: updated });
		const result = await setMainCharacter(fetch, BACKEND, 'c1', COOKIE);
		expect(result).toEqual(updated);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/characters/c1/set-main`,
			expect.objectContaining({ method: 'POST', headers: { cookie: COOKIE } })
		);
	});
});

describe('api keys', () => {
	it('listKeys returns the unwrapped array on 200', async () => {
		const keys: KeyMetadataDto[] = [
			{ id: 'k1', name: 'ci', scope: 'account', expires_at: null, created_at: 'now' }
		];
		const fetch = mockJsonFetch(200, { data: keys });
		const result = await listKeys(fetch, BACKEND, COOKIE);
		expect(result).toEqual(keys);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/keys`,
			expect.objectContaining({ headers: { cookie: COOKIE } })
		);
	});

	it('createKey POSTs JSON and returns the plaintext key envelope on 201', async () => {
		const created: CreatedKeyDto = {
			id: 'k1',
			key: 'erb_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
			name: 'ci',
			expires_at: null,
			created_at: 'now'
		};
		const fetch = mockJsonFetch(201, { data: created });
		const result = await createKey(fetch, BACKEND, { name: 'ci', expires_at: null }, COOKIE);
		expect(result).toEqual(created);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/keys`,
			expect.objectContaining({
				method: 'POST',
				headers: { cookie: COOKIE, 'content-type': 'application/json' },
				body: JSON.stringify({ name: 'ci', expires_at: null })
			})
		);
	});

	it('createKey throws ApiError on duplicate name (409)', async () => {
		const fetch = mockJsonFetch(409, {
			error: { code: 'conflict', message: 'duplicate name' }
		});
		await expect(
			createKey(fetch, BACKEND, { name: 'ci', expires_at: null }, COOKIE)
		).rejects.toMatchObject({ code: 'conflict', status: 409 });
	});

	it('deleteKey returns undefined on 204 (no body)', async () => {
		const fetch = mockNoContentFetch();
		await expect(deleteKey(fetch, BACKEND, 'k1', COOKIE)).resolves.toBeUndefined();
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/keys/k1`,
			expect.objectContaining({ method: 'DELETE', headers: { cookie: COOKIE } })
		);
	});

	it('deleteKey throws ApiError on 404', async () => {
		const fetch = mockJsonFetch(404, { error: { code: 'not_found', message: 'gone' } });
		await expect(deleteKey(fetch, BACKEND, 'k1', COOKIE)).rejects.toMatchObject({
			code: 'not_found',
			status: 404
		});
	});
});

describe('maps client', () => {
	const aMap: MapDto = {
		id: 'm1',
		name: 'Delve',
		slug: 'delve',
		owner_account_id: 'acc1',
		description: null,
		acls: [],
		created_at: 'now',
		updated_at: 'now'
	};

	it('listMaps unwraps the array and forwards the cookie', async () => {
		const fetch = mockJsonFetch(200, { data: [aMap] });
		const result = await listMaps(fetch, BACKEND, COOKIE);
		expect(result).toEqual([aMap]);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps`,
			expect.objectContaining({ headers: { cookie: COOKIE } })
		);
	});

	it('createMap POSTs the body', async () => {
		const fetch = mockJsonFetch(201, { data: aMap });
		const body = { name: 'Delve', slug: 'delve', description: null };
		const result = await createMap(fetch, BACKEND, body, COOKIE);
		expect(result).toEqual(aMap);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps`,
			expect.objectContaining({
				method: 'POST',
				headers: { cookie: COOKIE, 'content-type': 'application/json' },
				body: JSON.stringify(body)
			})
		);
	});

	it('updateMap PATCHes by id', async () => {
		const fetch = mockJsonFetch(200, { data: aMap });
		await updateMap(fetch, BACKEND, 'm1', { name: 'D', slug: 'd', description: null }, COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps/m1`,
			expect.objectContaining({ method: 'PATCH' })
		);
	});

	it('deleteMap DELETEs by id and returns undefined on 204', async () => {
		const fetch = mockNoContentFetch();
		await expect(deleteMap(fetch, BACKEND, 'm1', COOKIE)).resolves.toBeUndefined();
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps/m1`,
			expect.objectContaining({ method: 'DELETE', headers: { cookie: COOKIE } })
		);
	});

	it('attachAcl POSTs acl_id to the map acls endpoint', async () => {
		const fetch = mockNoContentFetch();
		await attachAcl(fetch, BACKEND, 'm1', 'acl9', COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps/m1/acls`,
			expect.objectContaining({ method: 'POST', body: JSON.stringify({ acl_id: 'acl9' }) })
		);
	});

	it('detachAcl DELETEs the acl from the map', async () => {
		const fetch = mockNoContentFetch();
		await detachAcl(fetch, BACKEND, 'm1', 'acl9', COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/maps/m1/acls/acl9`,
			expect.objectContaining({ method: 'DELETE' })
		);
	});

	it('surfaces a slug conflict as an ApiError', async () => {
		const fetch = mockJsonFetch(409, { error: { code: 'slug_taken', message: 'taken' } });
		await expect(
			createMap(fetch, BACKEND, { name: 'x', slug: 'x', description: null }, COOKIE)
		).rejects.toMatchObject({ code: 'slug_taken', status: 409 });
	});
});

describe('acls client', () => {
	const anAcl: AclDto = {
		id: 'acl1',
		name: 'Friends',
		owner_account_id: 'acc1',
		created_at: 'now',
		updated_at: 'now'
	};
	const aMember: AclMemberDto = {
		id: 'mem1',
		acl_id: 'acl1',
		member_type: 'character',
		eve_entity_id: null,
		character_id: 'char-uuid',
		name: 'Pilot',
		permission: 'read',
		created_at: 'now',
		updated_at: 'now'
	};

	it('listAcls unwraps the array', async () => {
		const fetch = mockJsonFetch(200, { data: [anAcl] });
		expect(await listAcls(fetch, BACKEND, COOKIE)).toEqual([anAcl]);
	});

	it('createAcl POSTs the name', async () => {
		const fetch = mockJsonFetch(201, { data: anAcl });
		await createAcl(fetch, BACKEND, 'Friends', COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/acls`,
			expect.objectContaining({ method: 'POST', body: JSON.stringify({ name: 'Friends' }) })
		);
	});

	it('renameAcl PATCHes the name by id', async () => {
		const fetch = mockJsonFetch(200, { data: anAcl });
		await renameAcl(fetch, BACKEND, 'acl1', 'Foes', COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/acls/acl1`,
			expect.objectContaining({ method: 'PATCH', body: JSON.stringify({ name: 'Foes' }) })
		);
	});

	it('addAclMember POSTs the member body to the members endpoint', async () => {
		const fetch = mockJsonFetch(201, { data: aMember });
		const body = { member_type: 'character', character_id: 'char-uuid', permission: 'read' };
		await addAclMember(fetch, BACKEND, 'acl1', body, COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/acls/acl1/members`,
			expect.objectContaining({ method: 'POST', body: JSON.stringify(body) })
		);
	});

	it('updateAclMember PATCHes the member permission', async () => {
		const fetch = mockJsonFetch(200, { data: aMember });
		await updateAclMember(fetch, BACKEND, 'acl1', 'mem1', { permission: 'admin' }, COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/acls/acl1/members/mem1`,
			expect.objectContaining({ method: 'PATCH', body: JSON.stringify({ permission: 'admin' }) })
		);
	});

	it('removeAclMember DELETEs the member', async () => {
		const fetch = mockNoContentFetch();
		await removeAclMember(fetch, BACKEND, 'acl1', 'mem1', COOKIE);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/acls/acl1/members/mem1`,
			expect.objectContaining({ method: 'DELETE' })
		);
	});
});

describe('searchEntities client', () => {
	const page: EntitySearchPageDto = {
		characters: [{ id: 'c-uuid', eve_character_id: 7, name: 'Pilot' }],
		corporations: [],
		alliances: [],
		unavailable: false
	};

	it('builds the q query param and unwraps the page', async () => {
		const fetch = mockJsonFetch(200, { data: page });
		const result = await searchEntities(fetch, BACKEND, 'pil ot', COOKIE);
		expect(result).toEqual(page);
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/entities/search?q=pil+ot`,
			expect.objectContaining({ headers: { cookie: COOKIE } })
		);
	});

	it('appends the categories query param when given', async () => {
		const fetch = mockJsonFetch(200, { data: page });
		await searchEntities(fetch, BACKEND, 'abc', COOKIE, 'character,corporation');
		expect(fetch).toHaveBeenCalledWith(
			`${BACKEND}/api/v1/entities/search?q=abc&categories=character%2Ccorporation`,
			expect.objectContaining({ headers: { cookie: COOKIE } })
		);
	});

	it('throws ApiError on a non-2xx', async () => {
		const fetch = mockJsonFetch(403, { error: { code: 'forbidden', message: 'no' } });
		await expect(searchEntities(fetch, BACKEND, 'abc', COOKIE)).rejects.toMatchObject({
			code: 'forbidden',
			status: 403
		});
	});
});

describe('getHealth', () => {
	const healthy: HealthResponse = {
		status: 'ok',
		version: '0.1.0',
		commit: 'abc1234',
		components: [{ name: 'db', status: 'ok' }]
	};

	it('returns the flat body on 200 (NOT unwrapped from a data envelope)', async () => {
		const fetch = mockJsonFetch(200, healthy);
		const result = await getHealth(fetch, BACKEND);
		expect(result).toEqual(healthy);
		// URL only — no RequestInit, so no cookie is forwarded.
		expect(fetch).toHaveBeenCalledWith(`${BACKEND}/api/health`);
	});

	it('throws ApiError on a non-ok response', async () => {
		const fetch = mockJsonFetch(503, {});
		await expect(getHealth(fetch, BACKEND)).rejects.toBeInstanceOf(ApiError);
		await expect(getHealth(fetch, BACKEND)).rejects.toMatchObject({ status: 503 });
	});
});

describe('ApiError', () => {
	it('carries code, message, and status', () => {
		const err = new ApiError('some_code', 'some message', 400);
		expect(err.code).toBe('some_code');
		expect(err.message).toBe('some message');
		expect(err.status).toBe(400);
		expect(err.name).toBe('ApiError');
		expect(err).toBeInstanceOf(Error);
	});
});
