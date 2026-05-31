import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listBlocks: vi.fn(),
		blockCharacter: vi.fn(),
		unblockCharacter: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listBlocks, blockCharacter, unblockCharacter, ApiError } = await import('$lib/api');
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
