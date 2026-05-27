// Server hooks. Paraglide resolves the active locale per request (cookie →
// Accept-Language → baseLocale) and runs the rest of the request inside that
// locale's async context, so `m.*` message functions and `getLocale()` resolve
// the right language during SSR. The resolved locale is written into
// `<html lang="…">` via the `%paraglide.lang%` placeholder in app.html, so the
// server-rendered page is in the correct language on first paint — no flash.
//
// There is deliberately no `reroute` hook / src/hooks.ts: the `url` strategy is
// not used (no /en/ path prefixes — see the i18n change's design.md), so there
// is no localized URL to de-localize.

import type { Handle } from '@sveltejs/kit';
import { paraglideMiddleware } from '$lib/paraglide/server';

export const handle: Handle = ({ event, resolve }) =>
	paraglideMiddleware(event.request, ({ request, locale }) => {
		event.request = request;
		return resolve(event, {
			transformPageChunk: ({ html }) => html.replace('%paraglide.lang%', locale)
		});
	});
