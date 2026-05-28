// Modal confirmation is client-side; ConfirmDialog is tested in src/lib/components/ConfirmDialog.test.ts. Server actions remain testable in isolation here.
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listKeys: vi.fn(),
		createKey: vi.fn(),
		deleteKey: vi.fn(),
		deleteAccount: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listKeys, createKey, deleteKey, deleteAccount, ApiError } = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.createKey>>[0];

function makeLoadEvent(opts: { cookie?: string }): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost', {
			headers: opts.cookie ? { cookie: opts.cookie } : {}
		})
	} as unknown as LoadEvent;
}

function makeActionEvent(opts: { cookie?: string; formData?: Record<string, string> }): ActionEvent {
	const body = new URLSearchParams(opts.formData ?? {});
	return {
		request: new Request('http://localhost', {
			method: 'POST',
			headers: {
				...(opts.cookie ? { cookie: opts.cookie } : {}),
				'content-type': 'application/x-www-form-urlencoded'
			},
			body: body.toString()
		}),
		fetch: vi.fn() as unknown as ActionEvent['fetch']
	} as unknown as ActionEvent;
}

beforeEach(() => {
	vi.mocked(listKeys).mockReset();
	vi.mocked(createKey).mockReset();
	vi.mocked(deleteKey).mockReset();
	vi.mocked(deleteAccount).mockReset();
});

describe('account/+page.server load', () => {
	it('forwards cookie and returns keys from the backend', async () => {
		vi.mocked(listKeys).mockResolvedValue([
			{ id: 'k1', name: 'ci', scope: 'account', expires_at: null, created_at: 'now' }
		]);

		const result = (await load(makeLoadEvent({ cookie: 'session=jwt' })))!;

		expect(result.keys).toHaveLength(1);
		expect(listKeys).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'session=jwt');
	});
});

describe('account/+page.server actions', () => {
	describe('createKey', () => {
		it('returns the created key on success', async () => {
			vi.mocked(createKey).mockResolvedValue({
				id: 'k1',
				key: 'erb_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
				name: 'ci',
				expires_at: null,
				created_at: 'now'
			});

			const result = await actions.createKey(
				makeActionEvent({ cookie: 'session=jwt', formData: { name: 'ci', expires_at: '' } })
			);

			expect(result).toMatchObject({ createdKey: { name: 'ci' } });
			expect(createKey).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				{ name: 'ci', expires_at: null },
				'session=jwt'
			);
		});

		it('forwards expires_at when provided', async () => {
			vi.mocked(createKey).mockResolvedValue({
				id: 'k1',
				key: 'erb_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
				name: 'ci',
				expires_at: '2027-01-01T00:00:00.000Z',
				created_at: 'now'
			});

			await actions.createKey(
				makeActionEvent({
					formData: { name: 'ci', expires_at: '2027-01-01T00:00:00.000Z' }
				})
			);

			expect(createKey).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				{ name: 'ci', expires_at: '2027-01-01T00:00:00.000Z' },
				''
			);
		});

		it('returns fail(400) when name is missing', async () => {
			const result = await actions.createKey(makeActionEvent({ formData: {} }));
			expect(result).toMatchObject({ status: 400, data: { code: 'bad_request' } });
		});

		it('returns fail(400) when name is whitespace only', async () => {
			const result = await actions.createKey(makeActionEvent({ formData: { name: '   ' } }));
			expect(result).toMatchObject({ status: 400, data: { code: 'bad_request' } });
		});

		it('returns fail with ApiError code/message on backend 4xx', async () => {
			vi.mocked(createKey).mockRejectedValue(new ApiError('conflict', 'duplicate name', 409));

			const result = await actions.createKey(makeActionEvent({ formData: { name: 'ci' } }));

			expect(result).toMatchObject({
				status: 409,
				data: { code: 'conflict', message: 'duplicate name' }
			});
		});

		it('returns fail(500) on non-ApiError', async () => {
			vi.mocked(createKey).mockRejectedValue(new Error('boom'));

			const result = await actions.createKey(makeActionEvent({ formData: { name: 'ci' } }));

			expect(result).toMatchObject({ status: 500, data: { code: 'internal_error' } });
		});
	});

	describe('revokeKey', () => {
		it('forwards cookie and returns void on 204', async () => {
			vi.mocked(deleteKey).mockResolvedValue(undefined);

			const result = await actions.revokeKey(
				makeActionEvent({ cookie: 'session=jwt', formData: { key_id: 'k1' } })
			);

			expect(result).toBeUndefined();
			expect(deleteKey).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				'k1',
				'session=jwt'
			);
		});

		it('returns fail(400) when key_id is missing', async () => {
			const result = await actions.revokeKey(makeActionEvent({ formData: {} }));
			expect(result).toMatchObject({ status: 400, data: { code: 'bad_request' } });
		});

		it('returns fail with ApiError code and keyId on backend 404', async () => {
			vi.mocked(deleteKey).mockRejectedValue(new ApiError('not_found', 'gone', 404));

			const result = await actions.revokeKey(makeActionEvent({ formData: { key_id: 'k1' } }));

			expect(result).toMatchObject({
				status: 404,
				data: { code: 'not_found', message: 'gone', keyId: 'k1' }
			});
		});

		it('returns fail(500) with keyId on non-ApiError', async () => {
			vi.mocked(deleteKey).mockRejectedValue(new Error('boom'));

			const result = await actions.revokeKey(makeActionEvent({ formData: { key_id: 'k1' } }));

			expect(result).toMatchObject({
				status: 500,
				data: { code: 'internal_error', keyId: 'k1' }
			});
		});
	});

	describe('deleteAccount', () => {
		it('redirects to /login on success', async () => {
			vi.mocked(deleteAccount).mockResolvedValue(undefined);

			await expect(
				actions.deleteAccount(makeActionEvent({ cookie: 'session=jwt' }))
			).rejects.toMatchObject({ status: 303, location: '/login' });

			expect(deleteAccount).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				'session=jwt'
			);
		});

		it('returns fail on backend rejection', async () => {
			vi.mocked(deleteAccount).mockRejectedValue(
				new ApiError(
					'cannot_remove_last_server_admin',
					'Cannot remove the last server administrator; promote another admin first',
					409
				)
			);

			const result = await actions.deleteAccount(makeActionEvent({}));

			expect(result).toMatchObject({
				status: 409,
				data: { code: 'cannot_remove_last_server_admin' }
			});
		});
	});
});
