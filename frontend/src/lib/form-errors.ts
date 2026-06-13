import { fail } from '@sveltejs/kit';
import { ApiError } from '$lib/api';

/**
 * Shared catch-block helper for form actions.
 *
 * The maps/ACLs actions all turn a thrown `ApiError` into a `fail(...)` that
 * carries the action tag, the backend error code, and its message; anything
 * else becomes a generic 500. This collapses ~10 identical catch blocks into
 * one call while keeping the `fail` payload byte-identical:
 *
 *   ApiError → fail(e.status, { action, code: e.code, message: e.message, ...extra })
 *   other    → fail(500,      { action, code: 'internal_error',
 *                               message: 'An unexpected error occurred', ...extra })
 *
 * `extra` carries the per-action context some call sites pass through (e.g. the
 * `id`/`aclId`/`memberId` echoed back so the page can re-target the error).
 *
 * Server-only: imports `fail`; call from `+page.server.ts` catch blocks.
 */
export function failFrom(action: string, e: unknown, extra?: Record<string, unknown>) {
	if (e instanceof ApiError) {
		return fail(e.status, { action, code: e.code, message: e.message, ...extra });
	}
	return fail(500, {
		action,
		code: 'internal_error',
		message: 'An unexpected error occurred',
		...extra
	});
}
