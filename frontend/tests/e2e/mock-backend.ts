/**
 * Minimal HTTP mock for the backend API used by E2E tests.
 *
 * Listens on http://127.0.0.1:9100 — matching the BACKEND_INTERNAL_URL set in
 * playwright.config.ts. Playwright's globalSetup starts this server before
 * the SvelteKit webServer process so the app can make authenticated requests
 * during the test run.
 *
 * Authentication model:
 *   The mock returns 401 for authenticated endpoints by default. Tests that
 *   need an authenticated session SHALL set a cookie via
 *   page.context().addCookies with name=session and any non-empty value
 *   before navigating; the mock accepts any cookie whose value is non-empty
 *   as a valid session. This lets login.spec.ts (which expects an
 *   unauthenticated state) and the authenticated suites share the same mock
 *   without interfering.
 *
 *   The admin suites set the session value to "admin-session": the mock then
 *   returns is_server_admin: true from /api/v1/me and serves the /api/v1/admin/*
 *   endpoints. Any other non-empty value is a non-admin session, for which the
 *   admin endpoints (and the /admin route group, server-side) 404.
 *
 * Seeded data (returned when authenticated):
 *   - Account: id "acc1", active
 *   - Characters:
 *       "char-main"  — Main Pilot   (is_main: true,  token_status: active)
 *       "char-alt"   — Jita Trader  (is_main: false, token_status: active)
 *   - API keys: empty list (so /account renders its empty state)
 */

import http from 'node:http';

const ADMIN_COOKIE_VALUE = 'admin-session';

function meResponse(isAdmin: boolean) {
	return {
		data: {
			account: {
				id: 'acc1',
				status: 'active',
				is_server_admin: isAdmin,
				created_at: '2025-01-01T00:00:00Z'
			},
			characters: ME_RESPONSE.data.characters
		}
	};
}

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

function sessionValue(req: http.IncomingMessage): string | null {
	const cookieHeader = req.headers['cookie'] ?? '';
	const match = cookieHeader.split(/;\s*/).find((c) => c.startsWith('session='));
	if (!match) return null;
	const value = match.slice('session='.length);
	return value.length > 0 ? value : null;
}

function hasValidSession(req: http.IncomingMessage): boolean {
	return sessionValue(req) !== null;
}

function isAdminSession(req: http.IncomingMessage): boolean {
	return sessionValue(req) === ADMIN_COOKIE_VALUE;
}

function readBody(req: http.IncomingMessage): Promise<string> {
	return new Promise((resolve) => {
		let data = '';
		req.on('data', (chunk) => (data += chunk));
		req.on('end', () => resolve(data));
	});
}

// Mutable admin state so grant→revoke and block→unblock e2e flows observe the
// change on reload. "acc1" is the seeded admin viewing the page; "acc2" (owner
// of "Promote Me") starts as a non-admin so it can be promoted then revoked.
type AdminAccount = {
	id: string;
	status: string;
	is_server_admin: boolean;
	created_at: string;
	characters: { eve_character_id: number; name: string; is_main: boolean }[];
};
const adminAccounts: AdminAccount[] = [
	{
		id: 'acc1',
		status: 'active',
		is_server_admin: true,
		created_at: '2025-01-01T00:00:00Z',
		characters: [{ eve_character_id: 1001, name: 'Main Pilot', is_main: true }]
	},
	{
		id: 'acc2',
		status: 'active',
		is_server_admin: false,
		created_at: '2025-02-01T00:00:00Z',
		characters: [{ eve_character_id: 2001, name: 'Promote Me', is_main: true }]
	}
];
let blocks: {
	eve_character_id: number;
	character_name: string | null;
	corporation_name: string | null;
	reason: string | null;
	blocked_by: string | null;
	blocked_at: string;
}[] = [];

const server = http.createServer(async (req, res) => {
	const url = req.url ?? '/';
	const method = req.method ?? 'GET';

	if (method === 'GET' && url === '/api/v1/me') {
		if (!hasValidSession(req)) {
			unauthorised(res);
			return;
		}
		json(res, 200, meResponse(isAdminSession(req)));
		return;
	}

	// ── /api/v1/admin/* — only an admin session is served; otherwise 404
	//    (mirroring the backend's AdminAccount extractor / non-disclosure). ──
	if (url.startsWith('/api/v1/admin/')) {
		if (!isAdminSession(req)) {
			notFound(res);
			return;
		}

		if (method === 'GET' && url === '/api/v1/admin/accounts') {
			json(res, 200, { data: adminAccounts });
			return;
		}

		if (method === 'GET' && url.startsWith('/api/v1/admin/characters/esi-search')) {
			const q = (new URL(url, 'http://x').searchParams.get('q') ?? '').toLowerCase();
			// A pilot the local index does not know, found only via ESI.
			const esiPool = [{ eve_character_id: 90000123, name: 'Esi Only Pilot' }];
			const results = esiPool
				.filter((c) => c.name.toLowerCase().includes(q))
				.map((c) => ({
					eve_character_id: c.eve_character_id,
					name: c.name,
					portrait_url: `https://images.evetech.net/characters/${c.eve_character_id}/portrait?size=128`,
					already_blocked: blocks.some((b) => b.eve_character_id === c.eve_character_id)
				}));
			json(res, 200, { data: { results, unavailable: false } });
			return;
		}

		if (method === 'GET' && url.startsWith('/api/v1/admin/characters/search')) {
			const q = (new URL(url, 'http://x').searchParams.get('q') ?? '').toLowerCase();
			const results = adminAccounts
				.flatMap((a) =>
					a.characters.map((c) => ({
						eve_character_id: c.eve_character_id,
						name: c.name,
						is_main: c.is_main,
						account_id: a.id,
						portrait_url: `https://images.evetech.net/characters/${c.eve_character_id}/portrait?size=128`,
						already_blocked: blocks.some((b) => b.eve_character_id === c.eve_character_id)
					}))
				)
				.filter((c) => c.name.toLowerCase().includes(q));
			json(res, 200, { data: results });
			return;
		}

		const grantMatch = url.match(/^\/api\/v1\/admin\/accounts\/([^/]+)\/grant-admin$/);
		if (method === 'POST' && grantMatch) {
			const acc = adminAccounts.find((a) => a.id === grantMatch[1]);
			if (!acc) {
				json(res, 404, { error: { code: 'not_found', message: 'Account not found' } });
				return;
			}
			acc.is_server_admin = true;
			noContent(res);
			return;
		}

		const revokeMatch = url.match(/^\/api\/v1\/admin\/accounts\/([^/]+)\/revoke-admin$/);
		if (method === 'POST' && revokeMatch) {
			const acc = adminAccounts.find((a) => a.id === revokeMatch[1]);
			if (!acc) {
				json(res, 404, { error: { code: 'not_found', message: 'Account not found' } });
				return;
			}
			if (adminAccounts.filter((a) => a.is_server_admin).length <= 1 && acc.is_server_admin) {
				json(res, 409, {
					error: { code: 'cannot_remove_last_server_admin', message: 'last admin' }
				});
				return;
			}
			acc.is_server_admin = false;
			noContent(res);
			return;
		}

		if (method === 'GET' && url === '/api/v1/admin/blocks') {
			json(res, 200, { data: blocks });
			return;
		}

		if (method === 'POST' && url === '/api/v1/admin/blocks') {
			const body = JSON.parse((await readBody(req)) || '{}');
			if (!blocks.some((b) => b.eve_character_id === body.eve_character_id)) {
				blocks.unshift({
					eve_character_id: body.eve_character_id,
					character_name: `Char ${body.eve_character_id}`,
					corporation_name: null,
					reason: body.reason ?? null,
					blocked_by: 'acc1',
					blocked_at: new Date().toISOString()
				});
			}
			noContent(res);
			return;
		}

		const unblockMatch = url.match(/^\/api\/v1\/admin\/blocks\/(\d+)$/);
		if (method === 'DELETE' && unblockMatch) {
			const id = Number(unblockMatch[1]);
			const before = blocks.length;
			blocks = blocks.filter((b) => b.eve_character_id !== id);
			if (blocks.length === before) {
				json(res, 404, { error: { code: 'not_found', message: 'Not blocked' } });
				return;
			}
			noContent(res);
			return;
		}

		if (method === 'GET' && url.startsWith('/api/v1/admin/audit')) {
			json(res, 200, { data: { entries: [], next_before: null } });
			return;
		}

		notFound(res);
		return;
	}

	// Public ESI proxy stand-ins for the /admin/blocks corp lookup (ESI_PUBLIC_BASE
	// points here in e2e). Public info, no auth.
	const esiCharMatch = url.match(/^\/esi\/characters\/(\d+)\/$/);
	if (method === 'GET' && esiCharMatch) {
		json(res, 200, { corporation_id: 500, name: `Char ${esiCharMatch[1]}` });
		return;
	}
	const esiCorpMatch = url.match(/^\/esi\/corporations\/(\d+)\/$/);
	if (method === 'GET' && esiCorpMatch) {
		json(res, 200, { name: 'Mock Corp' });
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

	// GET /api/v1/keys — authenticated; returns an empty list so /account
	// renders its empty state. The e2e suite doesn't need a populated list
	// to exercise the danger-zone modal.
	if (method === 'GET' && url === '/api/v1/keys') {
		if (!hasValidSession(req)) {
			unauthorised(res);
			return;
		}
		json(res, 200, { data: [] });
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
