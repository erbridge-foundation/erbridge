import { describe, it, expect } from 'vitest';
import { permissionsFor, isPermissionAllowed } from './acl-permissions';

describe('permissionsFor', () => {
	it('offers manage and admin for characters', () => {
		const perms = permissionsFor('character');
		expect(perms).toContain('manage');
		expect(perms).toContain('admin');
		expect(perms).toEqual(['read', 'read_write', 'manage', 'admin', 'deny']);
	});

	it('withholds manage and admin for corporations', () => {
		const perms = permissionsFor('corporation');
		expect(perms).not.toContain('manage');
		expect(perms).not.toContain('admin');
		expect(perms).toEqual(['read', 'read_write', 'deny']);
	});

	it('withholds manage and admin for alliances', () => {
		const perms = permissionsFor('alliance');
		expect(perms).not.toContain('manage');
		expect(perms).not.toContain('admin');
	});
});

describe('isPermissionAllowed', () => {
	it('allows base permissions for every member type', () => {
		for (const t of ['character', 'corporation', 'alliance'] as const) {
			expect(isPermissionAllowed(t, 'read')).toBe(true);
			expect(isPermissionAllowed(t, 'read_write')).toBe(true);
			expect(isPermissionAllowed(t, 'deny')).toBe(true);
		}
	});

	it('allows manage/admin only for characters', () => {
		expect(isPermissionAllowed('character', 'manage')).toBe(true);
		expect(isPermissionAllowed('character', 'admin')).toBe(true);
		expect(isPermissionAllowed('corporation', 'manage')).toBe(false);
		expect(isPermissionAllowed('corporation', 'admin')).toBe(false);
		expect(isPermissionAllowed('alliance', 'manage')).toBe(false);
		expect(isPermissionAllowed('alliance', 'admin')).toBe(false);
	});
});
