// keep in sync with: backend/src/dto/account.rs
export type TokenStatus = 'active' | 'expired';

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
	locale: 'en' | 'de';
}

export type PreferencesPatch = Partial<PreferencesDto>;

// keep in sync with: backend/src/dto/admin.rs
export interface AdminAccountCharacterDto {
	eve_character_id: number;
	name: string;
	is_main: boolean;
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
	before?: string;
	limit?: number;
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
	if (query.before) params.set('before', query.before);
	if (query.limit !== undefined) params.set('limit', String(query.limit));
	const qs = params.toString();
	const url = `${backendUrl}/api/v1/admin/audit${qs ? `?${qs}` : ''}`;
	return request<AuditLogPageDto>(fetch, url, { headers: { cookie } });
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
