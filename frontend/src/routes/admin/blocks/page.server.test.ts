import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listBlocks: vi.fn(),
		blockCharacter: vi.fn(),
		unblockCharacter: vi.fn(),
		searchCharacters: vi.fn(),
		searchCharactersEsi: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listBlocks, blockCharacter, unblockCharacter, searchCharacters, searchCharactersEsi, ApiError } =
	await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.block>>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/admin/blocks', { headers: { cookie } })
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
	vi.mocked(listBlocks).mockReset();
	vi.mocked(blockCharacter).mockReset();
	vi.mocked(unblockCharacter).mockReset();
	vi.mocked(searchCharacters).mockReset();
	vi.mocked(searchCharactersEsi).mockReset();
});

describe('admin/blocks load', () => {
	it('returns the block list', async () => {
		vi.mocked(listBlocks).mockResolvedValue([
			{
				eve_character_id: 42,
				character_name: 'Griefer',
				corporation_name: 'Bad Corp',
				reason: 'spam',
				blocked_by: 'a1',
				blocked_at: 'now'
			}
		]);
		const result = (await load(makeLoadEvent()))!;
		expect(result.blocks).toHaveLength(1);
	});
});

describe('admin/blocks block action', () => {
	it('parses the id and optional reason and calls the backend', async () => {
		vi.mocked(blockCharacter).mockResolvedValue(undefined);
		const result = await actions.block(makeActionEvent({ eve_character_id: '90000001', reason: 'griefing' }));
		expect(result).toBeUndefined();
		expect(blockCharacter).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ eve_character_id: 90000001, reason: 'griefing' },
			'session=jwt'
		);
	});

	it('sends reason: null when blank', async () => {
		vi.mocked(blockCharacter).mockResolvedValue(undefined);
		await actions.block(makeActionEvent({ eve_character_id: '5', reason: '   ' }));
		expect(blockCharacter).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ eve_character_id: 5, reason: null },
			'session=jwt'
		);
	});

	it('returns fail(400) for a non-numeric id', async () => {
		const result = await actions.block(makeActionEvent({ eve_character_id: 'abc' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'block', code: 'bad_request' } });
		expect(blockCharacter).not.toHaveBeenCalled();
	});

	it('returns fail(400) for a non-positive id', async () => {
		const result = await actions.block(makeActionEvent({ eve_character_id: '0' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'block', code: 'bad_request' } });
	});

	it('surfaces the self-block 409', async () => {
		vi.mocked(blockCharacter).mockRejectedValue(new ApiError('cannot_block_self', 'no self block', 409));
		const result = await actions.block(makeActionEvent({ eve_character_id: '7' }));
		expect(result).toMatchObject({ status: 409, data: { action: 'block', code: 'cannot_block_self' } });
	});
});

describe('admin/blocks unblock action', () => {
	it('unblocks by character id', async () => {
		vi.mocked(unblockCharacter).mockResolvedValue(undefined);
		const result = await actions.unblock(makeActionEvent({ eve_character_id: '42' }));
		expect(result).toBeUndefined();
		expect(unblockCharacter).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			42,
			'session=jwt'
		);
	});

	it('surfaces a 404 with the eveCharacterId echoed back', async () => {
		vi.mocked(unblockCharacter).mockRejectedValue(new ApiError('not_found', 'not blocked', 404));
		const result = await actions.unblock(makeActionEvent({ eve_character_id: '42' }));
		expect(result).toMatchObject({
			status: 404,
			data: { action: 'unblock', code: 'not_found', eveCharacterId: 42 }
		});
	});
});

describe('admin/blocks search action (local DB)', () => {
	it('returns local results for a ≥3-char query', async () => {
		vi.mocked(searchCharacters).mockResolvedValue([
			{
				eve_character_id: 7,
				name: 'Wasp 223',
				is_main: true,
				account_id: 'acc-1',
				portrait_url: 'https://images.evetech.net/characters/7/portrait?size=128',
				already_blocked: false
			}
		]);
		const result = await actions.search(makeActionEvent({ q: 'wasp' }));
		expect(result).toMatchObject({ action: 'search', query: 'wasp' });
		expect((result as { results: unknown[] }).results).toHaveLength(1);
	});

	it('rejects a query shorter than 3 chars with too_short', async () => {
		const result = await actions.search(makeActionEvent({ q: 'wa' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'search', code: 'too_short' } });
		expect(searchCharacters).not.toHaveBeenCalled();
	});
});

describe('admin/blocks esiSearch action (fallback)', () => {
	it('passes through results and the unavailable indicator', async () => {
		vi.mocked(searchCharactersEsi).mockResolvedValue({
			results: [
				{
					eve_character_id: 99,
					name: 'Never Seen',
					portrait_url: 'https://images.evetech.net/characters/99/portrait?size=128',
					already_blocked: false
				}
			],
			unavailable: false
		});
		const result = await actions.esiSearch(makeActionEvent({ q: 'never' }));
		expect(result).toMatchObject({ action: 'esiSearch', query: 'never', unavailable: false });
		expect((result as { results: unknown[] }).results).toHaveLength(1);
	});

	it('surfaces unavailable=true gracefully (empty results)', async () => {
		vi.mocked(searchCharactersEsi).mockResolvedValue({ results: [], unavailable: true });
		const result = await actions.esiSearch(makeActionEvent({ q: 'never' }));
		expect(result).toMatchObject({ action: 'esiSearch', unavailable: true });
		expect((result as { results: unknown[] }).results).toHaveLength(0);
	});

	it('rejects a query shorter than 3 chars', async () => {
		const result = await actions.esiSearch(makeActionEvent({ q: 'ne' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'esiSearch', code: 'too_short' } });
		expect(searchCharactersEsi).not.toHaveBeenCalled();
	});
});

describe('admin/blocks corpLookup action', () => {
	it('resolves the corp name from public ESI (best-effort)', async () => {
		const fetchMock = vi
			.fn()
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ corporation_id: 500 }), { status: 200 })
			)
			.mockResolvedValueOnce(
				new Response(JSON.stringify({ name: 'Test Corp' }), { status: 200 })
			);
		const event = {
			request: new Request('http://localhost', {
				method: 'POST',
				headers: { 'content-type': 'application/x-www-form-urlencoded' },
				body: new URLSearchParams({ eve_character_id: '7' }).toString()
			}),
			fetch: fetchMock
		} as unknown as ActionEvent;

		const result = await actions.corpLookup(event);
		expect(result).toMatchObject({
			action: 'corpLookup',
			eve_character_id: 7,
			corporation_name: 'Test Corp'
		});
	});

	it('returns null corp when ESI is unreachable (best-effort)', async () => {
		const fetchMock = vi.fn().mockRejectedValue(new Error('network'));
		const event = {
			request: new Request('http://localhost', {
				method: 'POST',
				headers: { 'content-type': 'application/x-www-form-urlencoded' },
				body: new URLSearchParams({ eve_character_id: '7' }).toString()
			}),
			fetch: fetchMock
		} as unknown as ActionEvent;

		const result = await actions.corpLookup(event);
		expect(result).toMatchObject({ action: 'corpLookup', corporation_name: null });
	});
});
