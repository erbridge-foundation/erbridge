import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listMaps: vi.fn(),
		createMap: vi.fn(),
		deleteMap: vi.fn(),
		createAcl: vi.fn(),
		addAclMember: vi.fn(),
		getMe: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listMaps, createMap, deleteMap, createAcl, addAclMember, getMe, ApiError } =
	await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.create>>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/maps', { headers: { cookie } })
	} as unknown as LoadEvent;
}

function makeActionEvent(formData: Record<string, string>, cookie = 'session=jwt'): ActionEvent {
	return {
		request: new Request('http://localhost', {
			method: 'POST',
			headers: { cookie, 'content-type': 'application/x-www-form-urlencoded' },
			body: new URLSearchParams(formData).toString()
		}),
		fetch: vi.fn() as unknown as ActionEvent['fetch']
	} as unknown as ActionEvent;
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
const anAcl = {
	id: 'acl-new',
	name: 'Delve',
	owner_account_id: 'acc1',
	created_at: 'now',
	updated_at: 'now'
};

beforeEach(() => {
	vi.mocked(listMaps).mockReset();
	vi.mocked(createMap).mockReset();
	vi.mocked(deleteMap).mockReset();
	vi.mocked(createAcl).mockReset();
	vi.mocked(addAclMember).mockReset();
	vi.mocked(getMe).mockReset();
});

describe('maps load', () => {
	it('returns the maps list', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		const result = (await load(makeLoadEvent())) as { maps: unknown[] };
		expect(result.maps).toHaveLength(1);
	});
});

describe('maps create action (plain)', () => {
	it('creates with trimmed name/slug and optional description, no ACL', async () => {
		vi.mocked(createMap).mockResolvedValue(aMap);
		const result = await actions.create(
			makeActionEvent({ name: '  Delve ', slug: ' delve ', description: ' big ' })
		);
		expect(result).toBeUndefined();
		expect(createAcl).not.toHaveBeenCalled();
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: 'big', acl_id: undefined },
			'session=jwt'
		);
	});

	it('fails 400 when name or slug is missing', async () => {
		const result = await actions.create(makeActionEvent({ name: '', slug: 'd' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'create', code: 'bad_request' } });
		expect(createMap).not.toHaveBeenCalled();
	});

	it('surfaces a slug conflict as fail', async () => {
		vi.mocked(createMap).mockRejectedValue(new ApiError('slug_taken', 'slug taken', 409));
		const result = await actions.create(makeActionEvent({ name: 'D', slug: 'd' }));
		expect(result).toMatchObject({ status: 409, data: { action: 'create', code: 'slug_taken' } });
	});
});

describe('maps create action (default ACL)', () => {
	it('creates an ACL named after the map, seeds the main char as admin, attaches it', async () => {
		vi.mocked(createAcl).mockResolvedValue(anAcl);
		vi.mocked(getMe).mockResolvedValue({
			account: { id: 'acc1', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: [
				{
					id: 'char-main-uuid',
					eve_character_id: 1001,
					name: 'Main Pilot',
					corporation_id: 1,
					corporation_name: 'Corp',
					alliance_id: null,
					alliance_name: null,
					is_main: true,
					portrait_url: 'https://x',
					token_status: 'active'
				}
			]
		});
		vi.mocked(addAclMember).mockResolvedValue({} as never);
		vi.mocked(createMap).mockResolvedValue(aMap);

		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toBeUndefined();
		expect(createAcl).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'Delve', 'session=jwt');
		expect(addAclMember).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl-new',
			{ member_type: 'character', character_id: 'char-main-uuid', name: 'Main Pilot', permission: 'admin' },
			'session=jwt'
		);
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: null, acl_id: 'acl-new' },
			'session=jwt'
		);
	});

	it('still creates + attaches the ACL when the account has no main (empty ACL)', async () => {
		vi.mocked(createAcl).mockResolvedValue(anAcl);
		vi.mocked(getMe).mockResolvedValue({
			account: { id: 'acc1', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: []
		});
		vi.mocked(createMap).mockResolvedValue(aMap);

		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toBeUndefined();
		expect(createAcl).toHaveBeenCalled();
		expect(addAclMember).not.toHaveBeenCalled();
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: null, acl_id: 'acl-new' },
			'session=jwt'
		);
	});

	it('seeding the owner member is best-effort: addAclMember failure still creates the map', async () => {
		vi.mocked(createAcl).mockResolvedValue(anAcl);
		vi.mocked(getMe).mockResolvedValue({
			account: { id: 'acc1', status: 'active', is_server_admin: false, created_at: 'now' },
			characters: [
				{
					id: 'char-main-uuid',
					eve_character_id: 1001,
					name: 'Main Pilot',
					corporation_id: 1,
					corporation_name: 'Corp',
					alliance_id: null,
					alliance_name: null,
					is_main: true,
					portrait_url: 'https://x',
					token_status: 'active'
				}
			]
		});
		vi.mocked(addAclMember).mockRejectedValue(new ApiError('boom', 'no', 500));
		vi.mocked(createMap).mockResolvedValue(aMap);

		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toBeUndefined();
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: null, acl_id: 'acl-new' },
			'session=jwt'
		);
	});

	it('surfaces a createAcl failure as fail before creating the map', async () => {
		vi.mocked(createAcl).mockRejectedValue(new ApiError('conflict', 'dup', 409));
		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toMatchObject({ status: 409, data: { action: 'create', code: 'conflict' } });
		expect(createMap).not.toHaveBeenCalled();
	});
});

describe('maps delete action', () => {
	it('deletes by id', async () => {
		vi.mocked(deleteMap).mockResolvedValue(undefined);
		const result = await actions.delete(makeActionEvent({ id: 'm1' }));
		expect(result).toBeUndefined();
		expect(deleteMap).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'm1', 'session=jwt');
	});

	it('fails 400 when id missing', async () => {
		const result = await actions.delete(makeActionEvent({}));
		expect(result).toMatchObject({ status: 400, data: { action: 'delete', code: 'bad_request' } });
		expect(deleteMap).not.toHaveBeenCalled();
	});

	it('surfaces a backend error with the id echoed back', async () => {
		vi.mocked(deleteMap).mockRejectedValue(new ApiError('forbidden', 'no', 403));
		const result = await actions.delete(makeActionEvent({ id: 'm1' }));
		expect(result).toMatchObject({ status: 403, data: { action: 'delete', code: 'forbidden', id: 'm1' } });
	});
});
