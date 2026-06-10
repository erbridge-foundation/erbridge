import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAcls: vi.fn(),
		createAcl: vi.fn(),
		renameAcl: vi.fn(),
		deleteAcl: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listAcls, createAcl, renameAcl, deleteAcl, ApiError } = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.create>>[0];

function makeLoadEvent(cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request('http://localhost/acls', { headers: { cookie } })
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

const anAcl = {
	id: 'acl1',
	name: 'Friends',
	owner_account_id: 'acc1',
	created_at: 'now',
	updated_at: 'now'
};

beforeEach(() => {
	vi.mocked(listAcls).mockReset();
	vi.mocked(createAcl).mockReset();
	vi.mocked(renameAcl).mockReset();
	vi.mocked(deleteAcl).mockReset();
});

describe('acls load', () => {
	it('returns the manageable ACLs', async () => {
		vi.mocked(listAcls).mockResolvedValue([anAcl]);
		const result = (await load(makeLoadEvent())) as { acls: unknown[] };
		expect(result.acls).toHaveLength(1);
	});
});

describe('acls create action', () => {
	it('creates with a trimmed name', async () => {
		vi.mocked(createAcl).mockResolvedValue(anAcl);
		const result = await actions.create(makeActionEvent({ name: '  Friends ' }));
		expect(result).toBeUndefined();
		expect(createAcl).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'Friends', 'session=jwt');
	});

	it('fails 400 on an empty name', async () => {
		const result = await actions.create(makeActionEvent({ name: '   ' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'create', code: 'bad_request' } });
		expect(createAcl).not.toHaveBeenCalled();
	});

	it('surfaces a backend conflict', async () => {
		vi.mocked(createAcl).mockRejectedValue(new ApiError('conflict', 'dup', 409));
		const result = await actions.create(makeActionEvent({ name: 'Friends' }));
		expect(result).toMatchObject({ status: 409, data: { action: 'create', code: 'conflict' } });
	});
});

describe('acls rename action', () => {
	it('renames by id', async () => {
		vi.mocked(renameAcl).mockResolvedValue(anAcl);
		const result = await actions.rename(makeActionEvent({ id: 'acl1', name: 'Foes' }));
		expect(result).toBeUndefined();
		expect(renameAcl).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl1',
			'Foes',
			'session=jwt'
		);
	});

	it('fails 400 with the id echoed back', async () => {
		const result = await actions.rename(makeActionEvent({ id: 'acl1', name: '' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'rename', code: 'bad_request', id: 'acl1' } });
	});
});

describe('acls delete action', () => {
	it('deletes by id', async () => {
		vi.mocked(deleteAcl).mockResolvedValue(undefined);
		const result = await actions.delete(makeActionEvent({ id: 'acl1' }));
		expect(result).toBeUndefined();
		expect(deleteAcl).toHaveBeenCalledWith(expect.anything(), 'http://backend:3000', 'acl1', 'session=jwt');
	});

	it('fails 400 when id missing', async () => {
		const result = await actions.delete(makeActionEvent({}));
		expect(result).toMatchObject({ status: 400, data: { action: 'delete', code: 'bad_request' } });
		expect(deleteAcl).not.toHaveBeenCalled();
	});
});
