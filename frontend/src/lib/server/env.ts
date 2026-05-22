import { env } from '$env/dynamic/private';

function require_env(key: string): string {
	const value = env[key];
	if (!value) {
		throw new Error(`Missing required environment variable: ${key}`);
	}
	return value;
}

export function backend_internal_url(): string {
	return require_env('BACKEND_INTERNAL_URL');
}
