import { redirect, isRedirect } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getMe, ApiError } from '$lib/api';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ fetch, url, locals, request }) => {
	const isLoginRoute = url.pathname === '/login';
	const cookie = request.headers.get('cookie') ?? '';

	try {
		const me = await getMe(fetch, backend_internal_url(), cookie);
		locals.me = me;

		if (isLoginRoute) {
			redirect(303, '/');
		}

		return { me, meError: null };
	} catch (e) {
		if (isRedirect(e)) throw e;

		locals.me = null;

		if (e instanceof ApiError && e.status === 401) {
			if (!isLoginRoute) {
				redirect(303, '/login');
			}
			return { me: null, meError: null };
		}

		if (isLoginRoute) {
			return { me: null, meError: null };
		}

		const message = e instanceof Error ? e.message : 'Unknown error';
		return { me: null, meError: message };
	}
};
