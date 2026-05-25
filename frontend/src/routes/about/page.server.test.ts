import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return { ...actual, getHealth: vi.fn() };
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { getHealth } = await import('$lib/api');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

function makeEvent(): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch']
	} as unknown as LoadEvent;
}

beforeEach(() => {
	vi.mocked(getHealth).mockReset();
});

describe('/about +page.server load', () => {
	it('returns health on success and null healthError', async () => {
		const health = {
			status: 'ok' as const,
			version: '0.1.0',
			commit: 'abc1234',
			components: [{ name: 'db', status: 'ok' as const }]
		};
		vi.mocked(getHealth).mockResolvedValue(health);

		const result = (await load(makeEvent()))!;

		expect(result).toEqual({ health, healthError: null });
	});

	it('returns null health with an error message when getHealth throws', async () => {
		vi.mocked(getHealth).mockRejectedValue(new Error('connection refused'));

		const result = (await load(makeEvent()))!;

		expect(result.health).toBeNull();
		expect(result.healthError).toEqual({ message: 'connection refused' });
	});

	it('handles a non-Error rejection with a fallback message', async () => {
		vi.mocked(getHealth).mockRejectedValue('boom');

		const result = (await load(makeEvent()))!;

		expect(result.health).toBeNull();
		expect(result.healthError).toEqual({ message: 'Unknown error' });
	});
});
