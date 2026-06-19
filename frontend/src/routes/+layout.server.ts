import { redirect, isRedirect } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMe, getPreferences, ApiError, type PreferencesDto } from '$lib/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ fetch, url, locals, request }) => {
	const isLoginRoute = url.pathname === '/login';
	// Public routes render without an authenticated session: a getMe 401 must not
	// redirect them to /login. The list is deliberately minimal — everything else
	// is gated. /login is public so unauthenticated visitors can sign in; /blocked
	// is the public information page a rejected/blocked login lands on (rendered
	// chrome-less, like /login — see +layout.svelte); /maps/_proto is the
	// disposable map-canvas sandbox (static fixture, no loader, no auth — see
	// build-map-canvas-prototype). The real /maps/[slug] stays gated.
	//
	// /about and /preferences are NOT public: both are reached only from the
	// authenticated user menu, and pre-login accessibility does not depend on the
	// /preferences route being reachable — the preference store hydrates from
	// localStorage via the inline bootstrap in app.html (and the login page's own
	// controls), independent of any page load.
	const isPublicRoute =
		isLoginRoute ||
		url.pathname === '/blocked' ||
		url.pathname === '/maps/_proto';
	const cookie = request.headers.get('cookie') ?? '';

	try {
		const me = await getMe(fetch, backend_internal_url(), cookie);
		locals.me = me;

		// Only /login bounces an already-authenticated user away.
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
