import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return { ...actual, listAuditLog: vi.fn() };
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAuditLog } = await import('$lib/api');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

function makeEvent(search = '', cookie = 'session=jwt'): LoadEvent {
	const url = new URL(`http://localhost/admin/audit${search}`);
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		url,
		request: new Request(url.toString(), { headers: { cookie } })
	} as unknown as LoadEvent;
}

beforeEach(() => {
	vi.mocked(listAuditLog).mockReset();
	vi.mocked(listAuditLog).mockResolvedValue({ entries: [], next_before: null });
});

describe('admin/audit load', () => {
	it('passes only the default limit when no filters are set', async () => {
		await load(makeEvent());
		expect(listAuditLog).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ limit: 50 },
			'session=jwt'
		);
	});

	it('forwards all supplied filters and the before cursor', async () => {
		await load(
			makeEvent(
				'?event_type=eve_character_blocked&target_name=boss&target_type=eve_character&target_id=42&actor=acc-1&before=2026-01-01T00:00:00Z'
			)
		);
		expect(listAuditLog).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{
				limit: 50,
				event_type: 'eve_character_blocked',
				target_name: 'boss',
				target_type: 'eve_character',
				target_id: '42',
				actor: 'acc-1',
				before: '2026-01-01T00:00:00Z'
			},
			'session=jwt'
		);
	});

	it('returns the page and the echoed filters for the form', async () => {
		vi.mocked(listAuditLog).mockResolvedValue({
			entries: [
				{
					id: 'e1',
					occurred_at: 'now',
					actor_account_id: null,
					actor_character_id: null,
					actor_character_name: null,
					event_type: 'blocked_login_rejected',
					details: {},
					target_type: null,
					target_id: null,
					target_name: null
				}
			],
			next_before: '2026-01-01T00:00:00Z'
		});
		const result = (await load(makeEvent('?target_name=boss')))!;
		expect(result.page.entries).toHaveLength(1);
		expect(result.page.next_before).toBe('2026-01-01T00:00:00Z');
		expect(result.filters.target_name).toBe('boss');
		expect(result.filters.event_type).toBe('');
	});
});
