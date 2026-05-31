import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAdminAccounts: vi.fn(),
		searchCharacters: vi.fn(),
		grantAdmin: vi.fn(),
		revokeAdmin: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAdminAccounts, searchCharacters, grantAdmin, revokeAdmin, ApiError } =
	await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.grant>>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/admin/admins', { headers: { cookie } })
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
	vi.mocked(grantAdmin).mockReset();
	vi.mocked(revokeAdmin).mockReset();
});

describe('admin/admins load', () => {
	it('returns only the admin accounts', async () => {
		vi.mocked(listAdminAccounts).mockResolvedValue([
			{ id: 'a1', status: 'active', is_server_admin: true, created_at: 'now', characters: [] },
			{ id: 'a2', status: 'active', is_server_admin: false, created_at: 'now', characters: [] }
		]);
		const result = (await load(makeLoadEvent()))!;
		expect(result.admins).toHaveLength(1);
		expect(result.admins[0].id).toBe('a1');
	});
});

describe('admin/admins search action', () => {
	it('returns matching characters with their owning account', async () => {
		vi.mocked(searchCharacters).mockResolvedValue([
			{
				eve_character_id: 1,
				name: 'Pilot One',
				is_main: true,
				account_id: 'acc-1',
				portrait_url: 'https://images.evetech.net/characters/1/portrait?size=128',
				already_blocked: false
			}
		]);
		const result = await actions.search(makeActionEvent({ q: 'pil' }));
		expect(result).toMatchObject({
			action: 'search',
			query: 'pil',
			results: [{ name: 'Pilot One', account_id: 'acc-1' }]
		});
		expect(searchCharacters).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'pil',
			'session=jwt'
		);
	});

	it('returns fail(400) for an empty query', async () => {
		const result = await actions.search(makeActionEvent({ q: '   ' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'search', code: 'bad_request' } });
	});
});

describe('admin/admins grant action (resolves account_id from the picked character)', () => {
	it('grants admin to the resolved account_id', async () => {
		vi.mocked(grantAdmin).mockResolvedValue(undefined);
		const result = await actions.grant(makeActionEvent({ account_id: 'acc-1' }));
		expect(result).toBeUndefined();
		expect(grantAdmin).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acc-1',
			'session=jwt'
		);
	});

	it('returns fail(400) when account_id is missing', async () => {
		const result = await actions.grant(makeActionEvent({}));
		expect(result).toMatchObject({ status: 400, data: { action: 'grant', code: 'bad_request' } });
	});

	it('surfaces a backend 404', async () => {
		vi.mocked(grantAdmin).mockRejectedValue(new ApiError('not_found', 'gone', 404));
		const result = await actions.grant(makeActionEvent({ account_id: 'acc-x' }));
		expect(result).toMatchObject({ status: 404, data: { action: 'grant', code: 'not_found' } });
	});
});

describe('admin/admins revoke action', () => {
	it('revokes admin from the account', async () => {
		vi.mocked(revokeAdmin).mockResolvedValue(undefined);
		const result = await actions.revoke(makeActionEvent({ account_id: 'acc-1' }));
		expect(result).toBeUndefined();
		expect(revokeAdmin).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acc-1',
			'session=jwt'
		);
	});

	it('surfaces the last-admin guard 409 with the accountId echoed back', async () => {
		vi.mocked(revokeAdmin).mockRejectedValue(
			new ApiError('cannot_remove_last_server_admin', 'last admin', 409)
		);
		const result = await actions.revoke(makeActionEvent({ account_id: 'acc-1' }));
		expect(result).toMatchObject({
			status: 409,
			data: { action: 'revoke', code: 'cannot_remove_last_server_admin', accountId: 'acc-1' }
		});
	});
});
