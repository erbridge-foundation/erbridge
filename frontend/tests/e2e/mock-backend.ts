/**
 * Minimal HTTP mock for the backend API used by E2E tests.
 *
 * Listens on http://127.0.0.1:9100 — matching the BACKEND_INTERNAL_URL set in
 * playwright.config.ts. Playwright's globalSetup starts this server before
 * the SvelteKit webServer process so the app can make authenticated requests
 * during the test run.
 *
 * Authentication model:
 *   The mock returns 401 for /api/v1/me by default. Tests that need an
 *   authenticated session SHALL set a cookie via page.context().addCookies
 *   with name=session and any non-empty value before navigating; the mock
 *   accepts any cookie whose value is non-empty as a valid session. This
 *   lets login.spec.ts (which expects an unauthenticated state) and
 *   characters-confirm-dialog.spec.ts (which needs auth) share the same
 *   mock without interfering.
 *
 * Seeded data (returned when authenticated):
 *   - Account: id "acc1", active
 *   - Characters:
 *       "char-main"  — Main Pilot   (is_main: true,  token_status: active)
 *       "char-alt"   — Jita Trader  (is_main: false, token_status: active)
 */

import http from 'node:http';

const ME_RESPONSE = {
	data: {
		account: {
			id: 'acc1',
			status: 'active',
			is_server_admin: false,
			created_at: '2025-01-01T00:00:00Z'
		},
		characters: [
			{
				id: 'char-main',
				eve_character_id: 1001,
				name: 'Main Pilot',
				corporation_id: 1,
				corporation_name: 'Test Corp',
				alliance_id: null,
				alliance_name: null,
				is_main: true,
				portrait_url: 'https://images.evetech.net/characters/1001/portrait?size=64',
				token_status: 'active'
			},
			{
				id: 'char-alt',
				eve_character_id: 1002,
				name: 'Jita Trader',
				corporation_id: 1,
				corporation_name: 'Test Corp',
				alliance_id: null,
				alliance_name: null,
				is_main: false,
				portrait_url: 'https://images.evetech.net/characters/1002/portrait?size=64',
				token_status: 'active'
			}
		]
	}
};

function json(res: http.ServerResponse, status: number, body: unknown) {
	const payload = JSON.stringify(body);
	res.writeHead(status, { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(payload) });
	res.end(payload);
}

function noContent(res: http.ServerResponse) {
	res.writeHead(204);
	res.end();
}

function notFound(res: http.ServerResponse) {
	json(res, 404, { error: { code: 'not_found', message: 'Not found' } });
}

function unauthorised(res: http.ServerResponse) {
	json(res, 401, { error: { code: 'unauthorised', message: 'No session' } });
}

function hasValidSession(req: http.IncomingMessage): boolean {
	const cookieHeader = req.headers['cookie'] ?? '';
	const match = cookieHeader.split(/;\s*/).find((c) => c.startsWith('session='));
	if (!match) return false;
	const value = match.slice('session='.length);
	return value.length > 0;
}

const server = http.createServer((req, res) => {
	const url = req.url ?? '/';
	const method = req.method ?? 'GET';

	if (method === 'GET' && url === '/api/v1/me') {
		if (!hasValidSession(req)) {
			unauthorised(res);
			return;
		}
		json(res, 200, ME_RESPONSE);
		return;
	}

	// DELETE /api/v1/characters/:id
	const charDeleteMatch = url.match(/^\/api\/v1\/characters\/([^/]+)$/);
	if (method === 'DELETE' && charDeleteMatch) {
		noContent(res);
		return;
	}

	// DELETE /api/v1/account
	if (method === 'DELETE' && url === '/api/v1/account') {
		noContent(res);
		return;
	}

	notFound(res);
});

export default async function globalSetup() {
	await new Promise<void>((resolve, reject) => {
		server.listen(9100, '127.0.0.1', () => resolve());
		server.once('error', reject);
	});

	return async () => {
		await new Promise<void>((resolve) => server.close(() => resolve()));
	};
}
