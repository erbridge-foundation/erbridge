import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listAcls: vi.fn(),
		listAclMembers: vi.fn(),
		addAclMember: vi.fn(),
		updateAclMember: vi.fn(),
		removeAclMember: vi.fn(),
		searchEntities: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const {
	listAcls,
	listAclMembers,
	addAclMember,
	updateAclMember,
	removeAclMember,
	searchEntities,
	ApiError
} = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.addMember>>[0];

function makeLoadEvent(id: string, cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request(`http://localhost/acls/${id}`, { headers: { cookie } }),
		params: { id }
	} as unknown as LoadEvent;
}

function makeActionEvent(
	id: string,
	formData: Record<string, string>,
	cookie = 'session=jwt'
): ActionEvent {
	return {
		request: new Request('http://localhost', {
			method: 'POST',
			headers: { cookie, 'content-type': 'application/x-www-form-urlencoded' },
			body: new URLSearchParams(formData).toString()
		}),
		fetch: vi.fn() as unknown as ActionEvent['fetch'],
		params: { id }
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
	vi.mocked(listAclMembers).mockReset();
	vi.mocked(addAclMember).mockReset();
	vi.mocked(updateAclMember).mockReset();
	vi.mocked(removeAclMember).mockReset();
	vi.mocked(searchEntities).mockReset();
});

describe('acls/[id] load', () => {
	it('resolves the ACL and returns its members', async () => {
		vi.mocked(listAcls).mockResolvedValue([anAcl]);
		vi.mocked(listAclMembers).mockResolvedValue([]);
		const result = (await load(makeLoadEvent('acl1'))) as {
			acl: { name: string };
			members: unknown[];
		};
		expect(result.acl.name).toBe('Friends');
		expect(result.members).toEqual([]);
	});

	it('throws 404 for an ACL the account cannot manage', async () => {
		vi.mocked(listAcls).mockResolvedValue([anAcl]);
		await expect(load(makeLoadEvent('acl-other'))).rejects.toMatchObject({ status: 404 });
		expect(listAclMembers).not.toHaveBeenCalled();
	});
});

describe('acls/[id] search action', () => {
	it('returns grouped results and the unavailable flag for a ≥3-char query', async () => {
		vi.mocked(searchEntities).mockResolvedValue({
			characters: [{ id: 'c-uuid', eve_character_id: 7, name: 'Wasp' }],
			corporations: [],
			alliances: [],
			unavailable: false
		});
		const result = await actions.search(makeActionEvent('acl1', { q: 'wasp' }));
		expect(result).toMatchObject({ action: 'search', query: 'wasp', unavailable: false });
		expect((result as { characters: unknown[] }).characters).toHaveLength(1);
	});

	it('passes through unavailable: true distinctly', async () => {
		vi.mocked(searchEntities).mockResolvedValue({
			characters: [],
			corporations: [],
			alliances: [],
			unavailable: true
		});
		const result = await actions.search(makeActionEvent('acl1', { q: 'wasp' }));
		expect(result).toMatchObject({ action: 'search', unavailable: true });
	});

	it('rejects a query shorter than 3 chars with too_short', async () => {
		const result = await actions.search(makeActionEvent('acl1', { q: 'wa' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'search', code: 'too_short' } });
		expect(searchEntities).not.toHaveBeenCalled();
	});
});

describe('acls/[id] addMember action — identifier by type', () => {
	const aMember = {
		id: 'mem1',
		acl_id: 'acl1',
		member_type: 'character',
		eve_entity_id: null,
		character_id: 'c-uuid',
		name: 'Wasp',
		permission: 'read',
		created_at: 'now',
		updated_at: 'now'
	};

	it('sends character_id for a character member', async () => {
		vi.mocked(addAclMember).mockResolvedValue(aMember);
		const result = await actions.addMember(
			makeActionEvent('acl1', {
				member_type: 'character',
				character_id: 'c-uuid',
				name: 'Wasp',
				permission: 'manage'
			})
		);
		expect(result).toBeUndefined();
		expect(addAclMember).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl1',
			{ member_type: 'character', permission: 'manage', name: 'Wasp', character_id: 'c-uuid' },
			'session=jwt'
		);
	});

	it('sends eve_entity_id for a corporation member', async () => {
		vi.mocked(addAclMember).mockResolvedValue({ ...aMember, member_type: 'corporation' });
		await actions.addMember(
			makeActionEvent('acl1', {
				member_type: 'corporation',
				eve_entity_id: '98000001',
				name: 'Corp',
				permission: 'read'
			})
		);
		expect(addAclMember).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl1',
			{ member_type: 'corporation', permission: 'read', name: 'Corp', eve_entity_id: 98000001 },
			'session=jwt'
		);
	});

	it('fails 400 when a character member has no character_id', async () => {
		const result = await actions.addMember(
			makeActionEvent('acl1', { member_type: 'character', permission: 'read' })
		);
		expect(result).toMatchObject({ status: 400, data: { action: 'addMember', code: 'bad_request' } });
		expect(addAclMember).not.toHaveBeenCalled();
	});

	it('fails 400 when a corporation member has a non-numeric eve_entity_id', async () => {
		const result = await actions.addMember(
			makeActionEvent('acl1', { member_type: 'corporation', eve_entity_id: 'abc', permission: 'read' })
		);
		expect(result).toMatchObject({ status: 400, data: { action: 'addMember', code: 'bad_request' } });
		expect(addAclMember).not.toHaveBeenCalled();
	});

	it('surfaces a backend CHECK rejection as a handled fail', async () => {
		vi.mocked(addAclMember).mockRejectedValue(new ApiError('invalid_permission', 'no admin for corp', 422));
		const result = await actions.addMember(
			makeActionEvent('acl1', { member_type: 'corporation', eve_entity_id: '5', permission: 'admin' })
		);
		expect(result).toMatchObject({ status: 422, data: { action: 'addMember', code: 'invalid_permission' } });
	});
});

describe('acls/[id] updateMember + removeMember actions', () => {
	it('updates a member permission by id', async () => {
		vi.mocked(updateAclMember).mockResolvedValue({} as never);
		const result = await actions.updateMember(
			makeActionEvent('acl1', { member_id: 'mem1', permission: 'admin' })
		);
		expect(result).toBeUndefined();
		expect(updateAclMember).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl1',
			'mem1',
			{ permission: 'admin' },
			'session=jwt'
		);
	});

	it('removes a member by id', async () => {
		vi.mocked(removeAclMember).mockResolvedValue(undefined);
		const result = await actions.removeMember(makeActionEvent('acl1', { member_id: 'mem1' }));
		expect(result).toBeUndefined();
		expect(removeAclMember).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'acl1',
			'mem1',
			'session=jwt'
		);
	});

	it('removeMember fails 400 when member_id missing', async () => {
		const result = await actions.removeMember(makeActionEvent('acl1', {}));
		expect(result).toMatchObject({ status: 400, data: { action: 'removeMember', code: 'bad_request' } });
		expect(removeAclMember).not.toHaveBeenCalled();
	});
});
