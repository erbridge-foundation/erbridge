import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listMaps: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listMaps } = await import('$lib/api');
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
	vi.mocked(listMaps).mockReset();
});

describe('maps/[slug] canvas load', () => {
	it('resolves a known slug', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		const result = (await load(makeLoadEvent('delve'))) as { map: { id: string } };
		expect(result.map.id).toBe('m1');
	});

	it('throws 404 for an unknown slug', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		await expect(load(makeLoadEvent('nope'))).rejects.toMatchObject({ status: 404 });
	});
});
