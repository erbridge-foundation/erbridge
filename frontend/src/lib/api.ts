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
