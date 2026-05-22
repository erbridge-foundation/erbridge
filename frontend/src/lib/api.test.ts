import { describe, it, expect, vi } from 'vitest';
import { ApiError, getMe, deleteCharacter, deleteAccount, setMainCharacter } from './api';
import type { MeResponse, CharacterDto } from './api';

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
