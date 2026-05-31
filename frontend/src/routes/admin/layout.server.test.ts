import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return { ...actual, getMe: vi.fn() };
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { getMe, ApiError } = await import('$lib/api');
const { load } = await import('./+layout.server');

type LoadEvent = Parameters<typeof load>[0];

function makeEvent(opts: { cookie?: string } = {}): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/admin', {
			headers: opts.cookie ? { cookie: opts.cookie } : {}
		})
	} as unknown as LoadEvent;
}

function adminMe(is_server_admin: boolean) {
	return {
		account: { id: 'a', status: 'active', is_server_admin, created_at: 'now' },
		characters: []
	};
}

beforeEach(() => {
	vi.mocked(getMe).mockReset();
});

describe('admin/+layout.server load (404-gates non-admins)', () => {
	it('returns for a server admin', async () => {
		vi.mocked(getMe).mockResolvedValue(adminMe(true));
		await expect(load(makeEvent({ cookie: 'session=jwt' }))).resolves.toEqual({});
	});

	it('404s an authenticated non-admin (does not disclose existence)', async () => {
		vi.mocked(getMe).mockResolvedValue(adminMe(false));
		await expect(load(makeEvent({ cookie: 'session=jwt' }))).rejects.toMatchObject({
			status: 404
		});
	});

	it('404s an unauthenticated caller (getMe 401)', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('unauthenticated', 'auth required', 401));
		await expect(load(makeEvent())).rejects.toMatchObject({ status: 404 });
	});

	it('404s when the backend is unreachable (fail-closed)', async () => {
		vi.mocked(getMe).mockRejectedValue(new Error('ECONNREFUSED'));
		await expect(load(makeEvent({ cookie: 'session=jwt' }))).rejects.toMatchObject({
			status: 404
		});
	});

	it('forwards the cookie header to getMe', async () => {
		vi.mocked(getMe).mockResolvedValue(adminMe(true));
		await load(makeEvent({ cookie: 'session=abc' }));
		expect(getMe).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'session=abc');
	});
});
