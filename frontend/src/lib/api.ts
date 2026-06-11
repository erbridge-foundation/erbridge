// keep in sync with: backend/src/dto/account.rs
// 'owner_mismatch' = the character was transferred to a different EVE account;
// the current owner cannot re-authenticate it, so it must be removed.
export type TokenStatus = 'active' | 'expired' | 'owner_mismatch';

export interface AccountDto {
	id: string;
	status: string;
	is_server_admin: boolean;
	created_at: string;
}

export interface CharacterDto {
	id: string;
	eve_character_id: number;
	name: string;
	corporation_id: number;
	corporation_name: string;
	alliance_id: number | null;
	alliance_name: string | null;
	is_main: boolean;
	portrait_url: string;
	token_status: TokenStatus;
}

export interface MeResponse {
	account: AccountDto;
	characters: CharacterDto[];
}

// keep in sync with: backend/src/dto/keys.rs
export interface CreatedKeyDto {
	id: string;
	key: string;
	name: string;
	expires_at: string | null;
	created_at: string;
}

export interface KeyMetadataDto {
	id: string;
	name: string;
	scope: string;
	expires_at: string | null;
	created_at: string;
}

export interface CreateKeyRequest {
	name: string;
	expires_at: string | null;
}

// keep in sync with: backend/src/dto/preferences.rs and lib/preferences/schema.ts
export interface PreferencesDto {
	text_size: 'auto' | 'small' | 'regular' | 'large';
	reduce_motion: 'auto' | 'on' | 'off';
	high_contrast: 'auto' | 'on' | 'off';
	large_targets: 'off' | 'on';
	dyslexia_font: 'off' | 'on';
	locale: 'en' | 'de' | 'fr';
}

export type PreferencesPatch = Partial<PreferencesDto>;

// keep in sync with: backend/src/dto/admin.rs
export interface AdminAccountCharacterDto {
	eve_character_id: number;
	name: string;
	is_main: boolean;
	token_status: TokenStatus;
}

export interface AdminAccountDto {
	id: string;
	status: string;
	is_server_admin: boolean;
	created_at: string;
	characters: AdminAccountCharacterDto[];
}

export interface CharacterSearchResultDto {
	eve_character_id: number;
	name: string;
	is_main: boolean;
	account_id: string | null;
	portrait_url: string;
	already_blocked: boolean;
}

export interface EsiCharacterSearchResultDto {
	eve_character_id: number;
	name: string;
	portrait_url: string;
	already_blocked: boolean;
}

export interface EsiCharacterSearchPageDto {
	results: EsiCharacterSearchResultDto[];
	unavailable: boolean;
}

export interface BlockedCharacterDto {
	eve_character_id: number;
	character_name: string | null;
	corporation_name: string | null;
	reason: string | null;
	blocked_by: string | null;
	blocked_at: string;
}

export interface AuditLogEntryDto {
	id: string;
	occurred_at: string;
	actor_account_id: string | null;
	actor_character_id: number | null;
	actor_character_name: string | null;
	event_type: string;
	details: unknown;
	target_type: string | null;
	target_id: string | null;
	target_name: string | null;
}

export interface AuditLogPageDto {
	entries: AuditLogEntryDto[];
	next_before: string | null;
}

export interface AuditLogQuery {
	event_type?: string;
	actor?: string;
	target_type?: string;
	target_id?: string;
	target_name?: string;
	/** Combined name search: actor OR target name, case-insensitive substring. */
	q?: string;
	/** Relative time window: 7d (default), 30d, 90d, 365d, or year:YYYY. */
	window?: string;
	/** Explicit RFC 3339 lower time bound; overrides window. */
	since?: string;
	before?: string;
	limit?: number;
}

// keep in sync with: backend/src/dto/map.rs
export interface AclSummaryDto {
	id: string;
	name: string;
}

export interface MapDto {
	id: string;
	name: string;
	slug: string;
	owner_account_id: string | null;
	description: string | null;
	acls: AclSummaryDto[];
	created_at: string;
	updated_at: string;
}

export interface CreateMapRequest {
	name: string;
	slug: string;
	description: string | null;
	acl_id?: string | null;
}

export interface UpdateMapRequest {
	name: string;
	slug: string;
	description: string | null;
}

// keep in sync with: backend/src/dto/acl.rs
export interface AclDto {
	id: string;
	name: string;
	owner_account_id: string | null;
	created_at: string;
	updated_at: string;
}

export interface AclMemberDto {
	id: string;
	acl_id: string;
	member_type: string;
	eve_entity_id: number | null;
	character_id: string | null;
	name: string;
	permission: string;
	created_at: string;
	updated_at: string;
}

export interface AddMemberRequest {
	member_type: string;
	eve_entity_id?: number | null;
	character_id?: string | null;
	name?: string;
	permission: string;
}

export interface UpdateMemberRequest {
	permission: string;
}

// keep in sync with: backend/src/dto/entity.rs
export interface EntityCharacterDto {
	id: string;
	eve_character_id: number;
	name: string;
}

export interface EntityOrgDto {
	eve_entity_id: number;
	name: string;
}

export interface EntitySearchPageDto {
	characters: EntityCharacterDto[];
	corporations: EntityOrgDto[];
	alliances: EntityOrgDto[];
	unavailable: boolean;
}

// keep in sync with: backend/src/dto/health.rs
export type HealthStatus = 'ok' | 'degraded';

export interface ComponentHealth {
	name: string;
	status: HealthStatus;
}

// Flat (unenveloped) document — /api/health is the api-contract carve-out.
export interface HealthResponse {
	status: HealthStatus;
	version: string;
	commit: string;
	components: ComponentHealth[];
}

// keep in sync with: backend/src/error.rs
export class ApiError extends Error {
	constructor(
		public readonly code: string,
		message: string,
		public readonly status: number
	) {
		super(message);
		this.name = 'ApiError';
	}
}

async function request<T>(
	fetch: typeof globalThis.fetch,
	url: string,
	init?: RequestInit
): Promise<T> {
	const res = await fetch(url, init);

	if (!res.ok) {
		let code = 'unknown_error';
		let message = res.statusText;
		try {
			const body = await res.json();
			code = body?.error?.code ?? code;
			message = body?.error?.message ?? message;
		} catch {
			// non-JSON error body — keep defaults
		}
		throw new ApiError(code, message, res.status);
	}

	if (res.status === 204) {
		return undefined as T;
	}

	const body = await res.json();
	return body.data as T;
}

export function getMe(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<MeResponse> {
	return request<MeResponse>(fetch, `${backendUrl}/api/v1/me`, {
		headers: { cookie }
	});
}

export function setMainCharacter(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	characterId: string,
	cookie: string
): Promise<CharacterDto> {
	return request<CharacterDto>(fetch, `${backendUrl}/api/v1/characters/${characterId}/set-main`, {
		method: 'POST',
		headers: { cookie }
	});
}

export function deleteCharacter(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	characterId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/characters/${characterId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function deleteAccount(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/account`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function listKeys(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<KeyMetadataDto[]> {
	return request<KeyMetadataDto[]>(fetch, `${backendUrl}/api/v1/keys`, {
		headers: { cookie }
	});
}

export function createKey(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	body: CreateKeyRequest,
	cookie: string
): Promise<CreatedKeyDto> {
	return request<CreatedKeyDto>(fetch, `${backendUrl}/api/v1/keys`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function deleteKey(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	keyId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/keys/${keyId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function getPreferences(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<PreferencesDto> {
	return request<PreferencesDto>(fetch, `${backendUrl}/api/v1/me/preferences`, {
		headers: { cookie }
	});
}

export function updatePreferences(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	patch: PreferencesPatch,
	cookie: string
): Promise<PreferencesDto> {
	return request<PreferencesDto>(fetch, `${backendUrl}/api/v1/me/preferences`, {
		method: 'PATCH',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(patch)
	});
}

// ── admin (session-cookie only) ────────────────────────────────────────────────

export function listAdminAccounts(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<AdminAccountDto[]> {
	return request<AdminAccountDto[]>(fetch, `${backendUrl}/api/v1/admin/accounts`, {
		headers: { cookie }
	});
}

export function searchCharacters(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	q: string,
	cookie: string
): Promise<CharacterSearchResultDto[]> {
	const url = `${backendUrl}/api/v1/admin/characters/search?q=${encodeURIComponent(q)}`;
	return request<CharacterSearchResultDto[]>(fetch, url, { headers: { cookie } });
}

export function searchCharactersEsi(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	q: string,
	cookie: string
): Promise<EsiCharacterSearchPageDto> {
	const url = `${backendUrl}/api/v1/admin/characters/esi-search?q=${encodeURIComponent(q)}`;
	return request<EsiCharacterSearchPageDto>(fetch, url, { headers: { cookie } });
}

export function grantAdmin(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	accountId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/admin/accounts/${accountId}/grant-admin`, {
		method: 'POST',
		headers: { cookie }
	});
}

export function revokeAdmin(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	accountId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/admin/accounts/${accountId}/revoke-admin`, {
		method: 'POST',
		headers: { cookie }
	});
}

export function listBlocks(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<BlockedCharacterDto[]> {
	return request<BlockedCharacterDto[]>(fetch, `${backendUrl}/api/v1/admin/blocks`, {
		headers: { cookie }
	});
}

export function blockCharacter(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	body: { eve_character_id: number; reason: string | null },
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/admin/blocks`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function unblockCharacter(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	eveCharacterId: number,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/admin/blocks/${eveCharacterId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function listAuditLog(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	query: AuditLogQuery,
	cookie: string
): Promise<AuditLogPageDto> {
	const params = new URLSearchParams();
	if (query.event_type) params.set('event_type', query.event_type);
	if (query.actor) params.set('actor', query.actor);
	if (query.target_type) params.set('target_type', query.target_type);
	if (query.target_id) params.set('target_id', query.target_id);
	if (query.target_name) params.set('target_name', query.target_name);
	if (query.q) params.set('q', query.q);
	if (query.window) params.set('window', query.window);
	if (query.since) params.set('since', query.since);
	if (query.before) params.set('before', query.before);
	if (query.limit !== undefined) params.set('limit', String(query.limit));
	const qs = params.toString();
	const url = `${backendUrl}/api/v1/admin/audit${qs ? `?${qs}` : ''}`;
	return request<AuditLogPageDto>(fetch, url, { headers: { cookie } });
}

// ── maps (account session) ─────────────────────────────────────────────────────

export function listMaps(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<MapDto[]> {
	return request<MapDto[]>(fetch, `${backendUrl}/api/v1/maps`, { headers: { cookie } });
}

export function createMap(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	body: CreateMapRequest,
	cookie: string
): Promise<MapDto> {
	return request<MapDto>(fetch, `${backendUrl}/api/v1/maps`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function getMap(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	mapId: string,
	cookie: string
): Promise<MapDto> {
	return request<MapDto>(fetch, `${backendUrl}/api/v1/maps/${mapId}`, { headers: { cookie } });
}

export function updateMap(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	mapId: string,
	body: UpdateMapRequest,
	cookie: string
): Promise<MapDto> {
	return request<MapDto>(fetch, `${backendUrl}/api/v1/maps/${mapId}`, {
		method: 'PATCH',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function deleteMap(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	mapId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/maps/${mapId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function attachAcl(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	mapId: string,
	aclId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/maps/${mapId}/acls`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify({ acl_id: aclId })
	});
}

export function detachAcl(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	mapId: string,
	aclId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/maps/${mapId}/acls/${aclId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

// ── ACLs (account session) ──────────────────────────────────────────────────────

export function listAcls(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	cookie: string
): Promise<AclDto[]> {
	return request<AclDto[]>(fetch, `${backendUrl}/api/v1/acls`, { headers: { cookie } });
}

export function createAcl(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	name: string,
	cookie: string
): Promise<AclDto> {
	return request<AclDto>(fetch, `${backendUrl}/api/v1/acls`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify({ name })
	});
}

export function renameAcl(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	name: string,
	cookie: string
): Promise<AclDto> {
	return request<AclDto>(fetch, `${backendUrl}/api/v1/acls/${aclId}`, {
		method: 'PATCH',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify({ name })
	});
}

export function deleteAcl(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/acls/${aclId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

export function listAclMembers(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	cookie: string
): Promise<AclMemberDto[]> {
	return request<AclMemberDto[]>(fetch, `${backendUrl}/api/v1/acls/${aclId}/members`, {
		headers: { cookie }
	});
}

export function addAclMember(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	body: AddMemberRequest,
	cookie: string
): Promise<AclMemberDto> {
	return request<AclMemberDto>(fetch, `${backendUrl}/api/v1/acls/${aclId}/members`, {
		method: 'POST',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function updateAclMember(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	memberId: string,
	body: UpdateMemberRequest,
	cookie: string
): Promise<AclMemberDto> {
	return request<AclMemberDto>(fetch, `${backendUrl}/api/v1/acls/${aclId}/members/${memberId}`, {
		method: 'PATCH',
		headers: { cookie, 'content-type': 'application/json' },
		body: JSON.stringify(body)
	});
}

export function removeAclMember(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	aclId: string,
	memberId: string,
	cookie: string
): Promise<void> {
	return request<void>(fetch, `${backendUrl}/api/v1/acls/${aclId}/members/${memberId}`, {
		method: 'DELETE',
		headers: { cookie }
	});
}

// ── entity search (account session) ─────────────────────────────────────────────

export function searchEntities(
	fetch: typeof globalThis.fetch,
	backendUrl: string,
	q: string,
	cookie: string,
	categories?: string
): Promise<EntitySearchPageDto> {
	const params = new URLSearchParams({ q });
	if (categories) params.set('categories', categories);
	const url = `${backendUrl}/api/v1/entities/search?${params.toString()}`;
	return request<EntitySearchPageDto>(fetch, url, { headers: { cookie } });
}

// /api/health is public and returns a flat (unenveloped) document, so it does
// NOT go through request() (which unwraps `body.data`) and forwards no cookie.
export async function getHealth(
	fetch: typeof globalThis.fetch,
	backendUrl: string
): Promise<HealthResponse> {
	const res = await fetch(`${backendUrl}/api/health`);
	if (!res.ok) {
		throw new ApiError('health_unavailable', `Health check failed: ${res.status}`, res.status);
	}
	return (await res.json()) as HealthResponse;
}
