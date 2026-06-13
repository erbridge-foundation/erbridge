import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		getMapBySlug: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { getMapBySlug, ApiError } = await import('$lib/api');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

function makeLoadEvent(slug: string, cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request(`http://localhost/maps/${slug}`, { headers: { cookie } }),
		params: { slug }
	} as unknown as LoadEvent;
}

const aMap = {
	id: 'm1',
	name: 'Delve',
	slug: 'delve',
	owner_account_id: 'acc1',
	description: null,
	acls: [],
	created_at: 'now',
	updated_at: 'now'
};

beforeEach(() => {
	vi.mocked(getMapBySlug).mockReset();
});

describe('maps/[slug] canvas load', () => {
	it('resolves a known slug', async () => {
		vi.mocked(getMapBySlug).mockResolvedValue(aMap);
		const result = (await load(makeLoadEvent('delve'))) as { map: { id: string } };
		expect(result.map.id).toBe('m1');
		expect(getMapBySlug).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'delve',
			'session=jwt'
		);
	});

	it('throws 404 when the backend 404s the slug', async () => {
		vi.mocked(getMapBySlug).mockRejectedValue(new ApiError('not_found', 'Map not found', 404));
		await expect(load(makeLoadEvent('nope'))).rejects.toMatchObject({ status: 404 });
	});
});
