import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		setMainCharacter: vi.fn(),
		deleteCharacter: vi.fn(),
		deleteAccount: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { setMainCharacter, deleteCharacter, deleteAccount, ApiError } = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type SetMainEvent = Parameters<NonNullable<typeof actions.setMain>>[0];

function makeActionEvent(opts: { cookie?: string; formData?: Record<string, string> }): SetMainEvent {
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
		fetch: vi.fn() as unknown as SetMainEvent['fetch']
	} as unknown as SetMainEvent;
}

beforeEach(() => {
	vi.mocked(setMainCharacter).mockReset();
	vi.mocked(deleteCharacter).mockReset();
	vi.mocked(deleteAccount).mockReset();
});

describe('characters/+page.server load', () => {
	it('returns characters from parent me', async () => {
		const parent = vi.fn().mockResolvedValue({
			me: { characters: [{ id: 'c1', name: 'Pilot' }] }
		});
		const result = (await load({ parent } as unknown as LoadEvent))!;
		expect(result.characters).toEqual([{ id: 'c1', name: 'Pilot' }]);
	});

	it('returns empty array when parent me is null', async () => {
		const parent = vi.fn().mockResolvedValue({ me: null });
		const result = (await load({ parent } as unknown as LoadEvent))!;
		expect(result.characters).toEqual([]);
	});
});

describe('characters/+page.server actions', () => {
	describe('setMain', () => {
		it('forwards cookie and returns void on success', async () => {
			vi.mocked(setMainCharacter).mockResolvedValue({
				id: 'c1',
				eve_character_id: 1,
				name: 'P',
				corporation_id: 1,
				corporation_name: 'C',
				alliance_id: null,
				alliance_name: null,
				is_main: true,
				portrait_url: '',
				token_status: 'active'
			});

			const result = await actions.setMain(
				makeActionEvent({ cookie: 'session=jwt', formData: { character_id: 'c1' } })
			);

			expect(result).toBeUndefined();
			expect(setMainCharacter).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				'c1',
				'session=jwt'
			);
		});

		it('returns fail(400) when character_id is missing', async () => {
			const result = await actions.setMain(makeActionEvent({ formData: {} }));
			expect(result).toMatchObject({ status: 400, data: { code: 'bad_request' } });
		});

		it('returns fail with ApiError code/message and characterId on backend 4xx', async () => {
			vi.mocked(setMainCharacter).mockRejectedValue(new ApiError('not_found', 'Resource not found', 404));

			const result = await actions.setMain(
				makeActionEvent({ formData: { character_id: 'c1' } })
			);

			expect(result).toMatchObject({
				status: 404,
				data: { code: 'not_found', message: 'Resource not found', characterId: 'c1' }
			});
		});

		it('returns fail(500) with internal_error code on non-ApiError', async () => {
			vi.mocked(setMainCharacter).mockRejectedValue(new Error('boom'));

			const result = await actions.setMain(
				makeActionEvent({ formData: { character_id: 'c1' } })
			);

			expect(result).toMatchObject({
				status: 500,
				data: { code: 'internal_error', characterId: 'c1' }
			});
		});
	});

	describe('remove', () => {
		it('forwards cookie and returns void on 204', async () => {
			vi.mocked(deleteCharacter).mockResolvedValue(undefined);

			const result = await actions.remove(
				makeActionEvent({ cookie: 'session=jwt', formData: { character_id: 'c1' } })
			);

			expect(result).toBeUndefined();
			expect(deleteCharacter).toHaveBeenCalledWith(
				expect.anything(),
				'http://backend:3000',
				'c1',
				'session=jwt'
			);
		});

		it('returns fail with cannot_remove_main when backend rejects main removal', async () => {
			vi.mocked(deleteCharacter).mockRejectedValue(
				new ApiError('cannot_remove_main', 'Cannot remove the main character', 409)
			);

			const result = await actions.remove(
				makeActionEvent({ formData: { character_id: 'c1' } })
			);

			expect(result).toMatchObject({
				status: 409,
				data: { code: 'cannot_remove_main', characterId: 'c1' }
			});
		});
	});

	describe('deleteAccount', () => {
		it('redirects to /login on success', async () => {
			vi.mocked(deleteAccount).mockResolvedValue(undefined);

			await expect(actions.deleteAccount(makeActionEvent({ cookie: 'session=jwt' }))).rejects.toMatchObject({
				status: 303,
				location: '/login'
			});

			expect(deleteAccount).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'session=jwt');
		});

		it('returns fail without characterId on backend rejection', async () => {
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
			expect((result as { data: { characterId?: string } }).data.characterId).toBeUndefined();
		});
	});
});
