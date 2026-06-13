import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listMaps: vi.fn(),
		createMap: vi.fn(),
		deleteMap: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listMaps, createMap, deleteMap, ApiError } = await import('$lib/api');
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
beforeEach(() => {
	vi.mocked(listMaps).mockReset();
	vi.mocked(createMap).mockReset();
	vi.mocked(deleteMap).mockReset();
});

describe('maps load', () => {
	it('returns the maps list', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		const result = (await load(makeLoadEvent())) as { maps: unknown[] };
		expect(result.maps).toHaveLength(1);
	});
});

describe('maps create action (plain)', () => {
	it('creates with trimmed name/slug and optional description, no default ACL', async () => {
		vi.mocked(createMap).mockResolvedValue(aMap);
		const result = await actions.create(
			makeActionEvent({ name: '  Delve ', slug: ' delve ', description: ' big ' })
		);
		expect(result).toBeUndefined();
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: 'big', default_acl: false },
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
	it('sends a single createMap with default_acl: true — no client-side orchestration', async () => {
		vi.mocked(createMap).mockResolvedValue(aMap);

		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toBeUndefined();
		// The backend mints + seeds + attaches the ACL atomically; the frontend
		// makes exactly one call and never touches createAcl/getMe/addAclMember.
		expect(createMap).toHaveBeenCalledTimes(1);
		expect(createMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			{ name: 'Delve', slug: 'delve', description: null, default_acl: true },
			'session=jwt'
		);
	});

	it('surfaces a slug conflict from the default-ACL create as fail (no orphan ACL leaks)', async () => {
		vi.mocked(createMap).mockRejectedValue(new ApiError('map_slug_already_exists', 'taken', 409));
		const result = await actions.create(
			makeActionEvent({ name: 'Delve', slug: 'delve', default_acl: 'on' })
		);
		expect(result).toMatchObject({
			status: 409,
			data: { action: 'create', code: 'map_slug_already_exists' }
		});
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
