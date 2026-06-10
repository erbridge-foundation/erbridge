import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('$lib/api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/api')>();
	return {
		...actual,
		listMaps: vi.fn(),
		listAcls: vi.fn(),
		updateMap: vi.fn(),
		attachAcl: vi.fn(),
		detachAcl: vi.fn()
	};
});

vi.mock('$lib/server/env', () => ({
	backend_internal_url: () => 'http://backend:3000'
}));

const { listMaps, listAcls, updateMap, attachAcl, detachAcl, ApiError } = await import('$lib/api');
const { load, actions } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<NonNullable<typeof actions.edit>>[0];

function makeLoadEvent(slug: string, cookie = 'session=jwt'): LoadEvent {
	return {
		fetch: vi.fn() as unknown as LoadEvent['fetch'],
		request: new Request(`http://localhost/maps/${slug}/settings`, { headers: { cookie } }),
		params: { slug }
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
	acls: [{ id: 'acl1', name: 'Friends' }],
	created_at: 'now',
	updated_at: 'now'
};
const anAcl = (id: string, name: string) => ({
	id,
	name,
	owner_account_id: 'acc1',
	created_at: 'now',
	updated_at: 'now'
});

beforeEach(() => {
	vi.mocked(listMaps).mockReset();
	vi.mocked(listAcls).mockReset();
	vi.mocked(updateMap).mockReset();
	vi.mocked(attachAcl).mockReset();
	vi.mocked(detachAcl).mockReset();
});

describe('maps/[slug]/settings load', () => {
	it('resolves the slug and offers only un-attached ACLs', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		vi.mocked(listAcls).mockResolvedValue([anAcl('acl1', 'Friends'), anAcl('acl2', 'Foes')]);
		const result = (await load(makeLoadEvent('delve'))) as {
			map: { id: string };
			attachable: { id: string }[];
		};
		expect(result.map.id).toBe('m1');
		expect(result.attachable.map((a) => a.id)).toEqual(['acl2']);
	});

	it('throws 404 for an unknown slug', async () => {
		vi.mocked(listMaps).mockResolvedValue([aMap]);
		await expect(load(makeLoadEvent('nope'))).rejects.toMatchObject({ status: 404 });
		expect(listAcls).not.toHaveBeenCalled();
	});
});

describe('maps/[slug]/settings edit action', () => {
	it('updates by id with description optional', async () => {
		vi.mocked(updateMap).mockResolvedValue(aMap);
		const result = await actions.edit(
			makeActionEvent({ id: 'm1', name: 'New', slug: 'new', description: '  ' })
		);
		expect(result).toBeUndefined();
		expect(updateMap).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'm1',
			{ name: 'New', slug: 'new', description: null },
			'session=jwt'
		);
	});

	it('fails 400 when fields missing', async () => {
		const result = await actions.edit(makeActionEvent({ id: 'm1', name: '', slug: 'x' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'edit', code: 'bad_request' } });
		expect(updateMap).not.toHaveBeenCalled();
	});

	it('surfaces a slug conflict', async () => {
		vi.mocked(updateMap).mockRejectedValue(new ApiError('slug_taken', 'taken', 409));
		const result = await actions.edit(makeActionEvent({ id: 'm1', name: 'N', slug: 'n' }));
		expect(result).toMatchObject({ status: 409, data: { action: 'edit', code: 'slug_taken' } });
	});
});

describe('maps/[slug]/settings attach action', () => {
	it('attaches the chosen ACL', async () => {
		vi.mocked(attachAcl).mockResolvedValue(undefined);
		const result = await actions.attach(makeActionEvent({ map_id: 'm1', acl_id: 'acl2' }));
		expect(result).toBeUndefined();
		expect(attachAcl).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'm1',
			'acl2',
			'session=jwt'
		);
	});

	it('fails 400 when no ACL chosen', async () => {
		const result = await actions.attach(makeActionEvent({ map_id: 'm1', acl_id: '' }));
		expect(result).toMatchObject({ status: 400, data: { action: 'attach', code: 'bad_request' } });
		expect(attachAcl).not.toHaveBeenCalled();
	});
});

describe('maps/[slug]/settings detach action', () => {
	it('detaches by acl id and echoes the id back on error', async () => {
		vi.mocked(detachAcl).mockResolvedValue(undefined);
		const ok = await actions.detach(makeActionEvent({ map_id: 'm1', acl_id: 'acl1' }));
		expect(ok).toBeUndefined();
		expect(detachAcl).toHaveBeenCalledWith(
			expect.anything(),
			'http://backend:3000',
			'm1',
			'acl1',
			'session=jwt'
		);

		vi.mocked(detachAcl).mockRejectedValue(new ApiError('not_found', 'gone', 404));
		const err = await actions.detach(makeActionEvent({ map_id: 'm1', acl_id: 'acl1' }));
		expect(err).toMatchObject({ status: 404, data: { action: 'detach', code: 'not_found', aclId: 'acl1' } });
	});
});
