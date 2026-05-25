import { redirect, isRedirect } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMe, getPreferences, ApiError, type PreferencesDto } from '$lib/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ fetch, url, locals, request }) => {
	const isLoginRoute = url.pathname === '/login';
	// Public routes render without an authenticated session: a getMe 401 must not
	// redirect them to /login. /about is intentionally public (its purpose is to be
	// findable); /login is public so unauthenticated visitors can sign in;
	// /preferences is public so accessibility settings work before/without login.
	const isPublicRoute =
		isLoginRoute || url.pathname === '/about' || url.pathname === '/preferences';
	const cookie = request.headers.get('cookie') ?? '';

	try {
		const me = await getMe(fetch, backend_internal_url(), cookie);
		locals.me = me;

		// Only /login bounces an already-authenticated user away; /about renders for everyone.
		if (isLoginRoute) {
			redirect(303, '/');
		}

		// Authenticated: fetch server-stored preferences so the client store can
		// reconcile localStorage against them. A failure here must not break the
		// page — preferences fall back to localStorage-only (serverPrefs: null).
		let serverPrefs: PreferencesDto | null = null;
		try {
			serverPrefs = await getPreferences(fetch, backend_internal_url(), cookie);
		} catch {
			serverPrefs = null;
		}

		return { me, meError: null, serverPrefs };
	} catch (e) {
		if (isRedirect(e)) throw e;

		locals.me = null;

		if (e instanceof ApiError && e.status === 401) {
			if (!isPublicRoute) {
				redirect(303, '/login');
			}
			return { me: null, meError: null, serverPrefs: null };
		}

		if (isPublicRoute) {
			return { me: null, meError: null, serverPrefs: null };
		}

		const message = e instanceof Error ? e.message : 'Unknown error';
		return { me: null, meError: message, serverPrefs: null };
	}
};
