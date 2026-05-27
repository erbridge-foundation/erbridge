import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return { ...actual, getPreferences: vi.fn(), updatePreferences: vi.fn() };
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { getPreferences, updatePreferences, ApiError } = await import('$lib/api');
const { GET, PATCH } = await import('./+server');

const DEFAULT_PREFS = {
	text_size: 'auto',
	reduce_motion: 'auto',
	high_contrast: 'auto',
	large_targets: 'off',
	dyslexia_font: 'off',
	locale: 'en'
} as const;

type Handler = typeof GET;

function event(opts: { cookie?: string; body?: unknown }): Parameters<Handler>[0] {
	return {
		fetch: vi.fn() as unknown as Parameters<Handler>[0]['fetch'],
		request: new Request('http://localhost/preferences', {
			method: opts.body !== undefined ? 'PATCH' : 'GET',
			headers: opts.cookie ? { cookie: opts.cookie } : {},
			body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined
		})
	} as unknown as Parameters<Handler>[0];
}

beforeEach(() => {
	vi.mocked(getPreferences).mockReset();
	vi.mocked(updatePreferences).mockReset();
});

describe('GET /preferences', () => {
	it('forwards the cookie and returns the enveloped data', async () => {
		vi.mocked(getPreferences).mockResolvedValue({ ...DEFAULT_PREFS, text_size: 'large' });
		const res = await GET(event({ cookie: 'session=abc' }));
		expect(getPreferences).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'session=abc');
		const body = await res.json();
		expect(body.data.text_size).toBe('large');
	});

	it('maps an ApiError status through', async () => {
		vi.mocked(getPreferences).mockRejectedValue(new ApiError('unauthenticated', 'no', 401));
		await expect(GET(event({ cookie: '' }))).rejects.toMatchObject({ status: 401 });
	});
});

describe('PATCH /preferences', () => {
	it('forwards the patch and cookie, returns merged data', async () => {
		vi.mocked(updatePreferences).mockResolvedValue({ ...DEFAULT_PREFS, reduce_motion: 'on' });
		const res = await PATCH(event({ cookie: 'session=abc', body: { reduce_motion: 'on' } }));
		expect(updatePreferences).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ reduce_motion: 'on' },
			'session=abc'
		);
		const body = await res.json();
		expect(body.data.reduce_motion).toBe('on');
	});

	it('maps a 400 from the backend through', async () => {
		vi.mocked(updatePreferences).mockRejectedValue(new ApiError('bad_request', 'bad', 400));
		await expect(
			PATCH(event({ cookie: 'session=abc', body: { bogus: 'x' } }))
		).rejects.toMatchObject({ status: 400 });
	});
});
