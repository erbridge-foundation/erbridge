// Browser → SvelteKit → backend proxy for preference sync.
//
// The preferences store runs in the browser (localStorage is the edge source of
// truth) but the backend is reachable only server-side (backend_internal_url is
// internal). This endpoint forwards the session cookie so authenticated users'
// preferences sync to the account. Anonymous users never call it — the store
// only syncs when a session exists.

import { json, error } from '@sveltejs/kit';
import { backend_internal_url } from '$lib/server/env';
import { getPreferences, updatePreferences, ApiError, type PreferencesPatch } from '$lib/api';
import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	try {
		const prefs = await getPreferences(fetch, backend_internal_url(), cookie);
		return json({ data: prefs });
	} catch (e) {
		if (e instanceof ApiError) error(e.status, e.message);
		error(500, 'Failed to load preferences');
	}
};

export const PATCH: RequestHandler = async ({ fetch, request }) => {
	const cookie = request.headers.get('cookie') ?? '';
	let patch: PreferencesPatch;
	try {
		patch = (await request.json()) as PreferencesPatch;
	} catch {
		error(400, 'Invalid JSON body');
	}

	try {
		const prefs = await updatePreferences(fetch, backend_internal_url(), patch, cookie);
		return json({ data: prefs });
	} catch (e) {
		if (e instanceof ApiError) error(e.status, e.message);
		error(500, 'Failed to update preferences');
	}
};
