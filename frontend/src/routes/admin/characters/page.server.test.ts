import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAdminAccounts: vi.fn(),
		searchCharacters: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAdminAccounts, searchCharacters, ApiError } = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.search>>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/admin/characters', { headers: { cookie } })
	} as unknown as LoadEvent;
}

function makeActionEvent(formData: Record<string, string>, cookie = 'session=jwt'): ActionEvent {
	const body = new URLSearchParams(formData);
	return {
		request: new Request('http://localhost', {
			method: 'POST',
			headers: { cookie, 'content-type': 'application/x-www-form-urlencoded' },
			body: body.toString()
		}),
		fetch: vi.fn() as unknown as ActionEvent['fetch']
	} as unknown as ActionEvent;
}

beforeEach(() => {
	vi.mocked(listAdminAccounts).mockReset();
	vi.mocked(searchCharacters).mockReset();
});

describe('admin/characters load', () => {
	it('returns all accounts (with characters + token_status) for the dialog', async () => {
		vi.mocked(listAdminAccounts).mockResolvedValue([
			{
				id: 'a1',
				status: 'active',
				is_server_admin: false,
				created_at: 'now',
				characters: [
					{ eve_character_id: 1, name: 'Main', is_main: true, token_status: 'active' },
					{ eve_character_id: 2, name: 'Sold', is_main: false, token_status: 'owner_mismatch' }
				]
			}
		]);
		const result = (await load(makeLoadEvent()))!;
		expect(result.accounts).toHaveLength(1);
		expect(result.accounts[0].characters[1].token_status).toBe('owner_mismatch');
		expect(listAdminAccounts).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'session=jwt'
		);
	});
});

describe('admin/characters search action', () => {
	it('returns results for a valid query', async () => {
		vi.mocked(searchCharacters).mockResolvedValue([
			{
				eve_character_id: 1,
				name: 'Pilot',
				is_main: true,
				account_id: 'a1',
				portrait_url: '',
				already_blocked: false
			}
		]);
		const result = await actions.search(makeActionEvent({ q: 'pil' }));
		expect(result).toMatchObject({ action: 'search', query: 'pil' });
		expect((result as { results: unknown[] }).results).toHaveLength(1);
	});

	it('returns fail(400) for an empty query', async () => {
		const result = await actions.search(makeActionEvent({ q: '   ' }));
		expect(result).toMatchObject({ status: 400, data: { code: 'bad_request' } });
		expect(searchCharacters).not.toHaveBeenCalled();
	});

	it('maps an ApiError to fail with its status/code', async () => {
		vi.mocked(searchCharacters).mockRejectedValue(new ApiError('forbidden', 'Nope', 403));
		const result = await actions.search(makeActionEvent({ q: 'x' }));
		expect(result).toMatchObject({ status: 403, data: { code: 'forbidden' } });
	});

	it('maps a non-ApiError to fail(500)', async () => {
		vi.mocked(searchCharacters).mockRejectedValue(new Error('boom'));
		const result = await actions.search(makeActionEvent({ q: 'x' }));
		expect(result).toMatchObject({ status: 500, data: { code: 'internal_error' } });
	});
});
