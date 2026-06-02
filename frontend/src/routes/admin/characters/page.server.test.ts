import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAdminAccounts: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAdminAccounts } = await import('$lib/api');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/admin/characters', { headers: { cookie } })
	} as unknown as LoadEvent;
}

beforeEach(() => {
	vi.mocked(listAdminAccounts).mockReset();
});

describe('admin/characters load', () => {
	it('returns all accounts (with characters + token_status) for the grid', async () => {
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
	});

	it('forwards the request cookie to the backend', async () => {
		vi.mocked(listAdminAccounts).mockResolvedValue([]);
		await load(makeLoadEvent('session=jwt'));
		expect(listAdminAccounts).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'session=jwt'
		);
	});

	it('passes an empty cookie string when none is present', async () => {
		vi.mocked(listAdminAccounts).mockResolvedValue([]);
		await load({
			fetch: vi.fn() as unknown as LoadEvent['fetch'],
			request: new Request('http://localhost/admin/characters')
		} as unknown as LoadEvent);
		expect(listAdminAccounts).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', '');
	});
});
