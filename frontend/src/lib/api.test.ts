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
	deleteKey
} from './api';
import type {
	MeResponse,
	CharacterDto,
	HealthResponse,
	KeyMetadataDto,
	CreatedKeyDto
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
