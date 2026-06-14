import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAdminAccounts: vi.fn(),
		hardDeletePreview: vi.fn(),
		hardDeleteAccount: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAdminAccounts, hardDeletePreview, hardDeleteAccount, ApiError } =
	await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.preview>>[0];

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
	vi.mocked(hardDeletePreview).mockReset();
	vi.mocked(hardDeleteAccount).mockReset();
});

describe('admin/characters load', () => {
	it('returns all accounts (with characters + token_status) for the grid', async () => {
		vi.mocked(listAdminAccounts).mockResolvedValue([
			{
				id: 'a1',
				status: 'active',
				is_server_admin: false,
				created_at: 'now',
				last_known_main_character_name: 'Main',
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

describe('admin/characters preview action', () => {
	const counts = { characters: 2, sessions: 1, api_keys: 0, owned_maps: 3, owned_acls: 1 };

	it('returns the blast-radius preview for the account', async () => {
		vi.mocked(hardDeletePreview).mockResolvedValue(counts);
		const result = await actions.preview(makeActionEvent({ account_id: 'a1' }));
		expect(result).toMatchObject({ action: 'preview', accountId: 'a1', preview: counts });
		expect(hardDeletePreview).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'a1',
			'session=jwt'
		);
	});

	it('fails 400 when account_id is missing', async () => {
		const result = await actions.preview(makeActionEvent({}));
		expect(result).toMatchObject({ status: 400, data: { action: 'preview', code: 'bad_request' } });
		expect(hardDeletePreview).not.toHaveBeenCalled();
	});

	it('surfaces a backend ApiError status', async () => {
		vi.mocked(hardDeletePreview).mockRejectedValue(new ApiError('not_found', 'gone', 404));
		const result = await actions.preview(makeActionEvent({ account_id: 'missing' }));
		expect(result).toMatchObject({ status: 404, data: { action: 'preview', code: 'not_found' } });
	});
});

describe('admin/characters delete action', () => {
	const removed = { characters: 1, sessions: 0, api_keys: 0, owned_maps: 0, owned_acls: 0 };

	it('hard-deletes the account and returns the removed counts', async () => {
		vi.mocked(hardDeleteAccount).mockResolvedValue(removed);
		const result = await actions.delete(makeActionEvent({ account_id: 'a1' }));
		expect(result).toMatchObject({ action: 'delete', accountId: 'a1', removed });
		expect(hardDeleteAccount).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'a1',
			'session=jwt'
		);
	});

	it('fails 400 when account_id is missing', async () => {
		const result = await actions.delete(makeActionEvent({}));
		expect(result).toMatchObject({ status: 400, data: { action: 'delete', code: 'bad_request' } });
		expect(hardDeleteAccount).not.toHaveBeenCalled();
	});

	it('surfaces the last-admin guard 409 as a failure', async () => {
		vi.mocked(hardDeleteAccount).mockRejectedValue(
			new ApiError('cannot_remove_last_server_admin', 'nope', 409)
		);
		const result = await actions.delete(makeActionEvent({ account_id: 'a1' }));
		expect(result).toMatchObject({
			status: 409,
			data: { action: 'delete', code: 'cannot_remove_last_server_admin' }
		});
	});
});
