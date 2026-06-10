// Pure permission-gating logic for ACL members, extracted so it is unit-testable
// independent of any component. The backend CHECK constraint is the authority;
// this is a UX guard (see openspec/changes/add-maps-and-acls-ui — "Client-side
// gate `manage`/`admin` to character members").

export type MemberType = 'character' | 'corporation' | 'alliance';
export type Permission = 'read' | 'read_write' | 'manage' | 'admin' | 'deny';

/** Permissions selectable for any member type. */
const BASE_PERMISSIONS: Permission[] = ['read', 'read_write', 'deny'];

/** Permissions selectable only for `character` members. */
const CHARACTER_ONLY_PERMISSIONS: Permission[] = ['manage', 'admin'];

/**
 * The permissions that may be offered for the given member type. `manage` and
 * `admin` are character-only; `read`, `read_write`, and `deny` apply to all.
 */
export function permissionsFor(memberType: MemberType): Permission[] {
	if (memberType === 'character') {
		return ['read', 'read_write', 'manage', 'admin', 'deny'];
	}
	return BASE_PERMISSIONS;
}

/** Whether a permission may be assigned to a member of the given type. */
export function isPermissionAllowed(memberType: MemberType, permission: Permission): boolean {
	if (CHARACTER_ONLY_PERMISSIONS.includes(permission)) {
		return memberType === 'character';
	}
	return true;
}
