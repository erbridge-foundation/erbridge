import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return { ...actual, getMe: vi.fn(), getPreferences: vi.fn() };
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { getMe, getPreferences, ApiError } = await import('$lib/api');
const { load } = await import('./+layout.server');

const DEFAULT_PREFS = {
	text_size: 'auto',
	reduce_motion: 'auto',
	high_contrast: 'auto',
	large_targets: 'off',
	dyslexia_font: 'off',
	locale: 'en'
} as const;

type LoadEvent = Parameters<typeof load>[0];

function makeEvent(opts: { pathname: string; cookie?: string }): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		url: new URL(`http://localhost${opts.pathname}`),
		locals: { me: null },
		request: new Request('http://localhost', {
			headers: opts.cookie ? { cookie: opts.cookie } : {}
		})
	} as unknown as LoadEvent;
}

beforeEach(() => {
	vi.mocked(getMe).mockReset();
	vi.mocked(getPreferences).mockReset();
	vi.mocked(getPreferences).mockResolvedValue({ ...DEFAULT_PREFS });
});

describe('+layout.server load', () => {
	it('returns me on success and clears meError', async () => {
		const me = {
			account: { id: 'a', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: []
		};
		vi.mocked(getMe).mockResolvedValue(me);
		const event = makeEvent({ pathname: '/', cookie: 'session=jwt' });

		const result = await load(event);

		expect(result).toEqual({ me, meError: null, serverPrefs: { ...DEFAULT_PREFS } });
		expect(event.locals.me).toBe(me);
	});

	it('forwards the cookie header to getMe', async () => {
		vi.mocked(getMe).mockResolvedValue({
			account: { id: '', status: '', is_server_admin: false, created_at: '' },
			characters: []
		});
		await load(makeEvent({ pathname: '/', cookie: 'session=abc' }));

		expect(getMe).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'session=abc');
	});

	it('passes empty string when no cookie present', async () => {
		vi.mocked(getMe).mockResolvedValue({
			account: { id: '', status: '', is_server_admin: false, created_at: '' },
			characters: []
		});
		await load(makeEvent({ pathname: '/' }));

		expect(getMe).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', '');
	});

	it('redirects authenticated user away from /login', async () => {
		vi.mocked(getMe).mockResolvedValue({
			account: { id: '', status: '', is_server_admin: false, created_at: '' },
			characters: []
		});

		await expect(load(makeEvent({ pathname: '/login', cookie: 'session=jwt' }))).rejects.toMatchObject({
			status: 303,
			location: '/'
		});
	});

	it('redirects unauthenticated user to /login on 401', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('unauthenticated', 'auth required', 401));

		await expect(load(makeEvent({ pathname: '/' }))).rejects.toMatchObject({
			status: 303,
			location: '/login'
		});
	});

	it('does NOT redirect unauthenticated user already on /login', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('unauthenticated', 'auth required', 401));

		const result = await load(makeEvent({ pathname: '/login' }));

		expect(result).toEqual({ me: null, meError: null, serverPrefs: null });
	});

	it('does NOT redirect unauthenticated user on /about (public route)', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('unauthenticated', 'auth required', 401));

		const result = await load(makeEvent({ pathname: '/about' }));

		expect(result).toEqual({ me: null, meError: null, serverPrefs: null });
	});

	it('does NOT bounce an authenticated user away from /about', async () => {
		const me = {
			account: { id: 'a', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: []
		};
		vi.mocked(getMe).mockResolvedValue(me);

		const result = (await load(makeEvent({ pathname: '/about', cookie: 'session=jwt' })))!;

		// Unlike /login, /about renders for authenticated users (no redirect to /).
		expect(result).toEqual({ me, meError: null, serverPrefs: { ...DEFAULT_PREFS } });
	});

	it('returns meError on non-401 error so the layout banner can render', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('bad_gateway', 'upstream failed', 502));

		const result = (await load(makeEvent({ pathname: '/' })))!;

		expect(result.me).toBeNull();
		expect(result.meError).toBe('upstream failed');
	});

	it('returns meError on non-ApiError network failure', async () => {
		vi.mocked(getMe).mockRejectedValue(new Error('ECONNREFUSED'));

		const result = (await load(makeEvent({ pathname: '/' })))!;

		expect(result.me).toBeNull();
		expect(result.meError).toBe('ECONNREFUSED');
	});

	it('on /login with non-401 error, returns null meError (no banner on login)', async () => {
		vi.mocked(getMe).mockRejectedValue(new ApiError('bad_gateway', 'upstream failed', 502));

		const result = await load(makeEvent({ pathname: '/login' }));

		expect(result).toEqual({ me: null, meError: null, serverPrefs: null });
	});

	it('returns serverPrefs:null when the preferences fetch fails (page still renders)', async () => {
		const me = {
			account: { id: 'a', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: []
		};
		vi.mocked(getMe).mockResolvedValue(me);
		vi.mocked(getPreferences).mockRejectedValue(new ApiError('bad_gateway', 'down', 502));

		const result = (await load(makeEvent({ pathname: '/', cookie: 'session=jwt' })))!;

		expect(result.me).toBe(me);
		expect(result.serverPrefs).toBeNull();
	});
});
