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

interface AuditEntry {
	id: string;
	occurred_at: string;
	actor_account_id: string | null;
	actor_character_id: number | null;
	actor_character_name: string | null;
	event_type: string;
	details: Record<string, unknown>;
	target_type: string | null;
	target_id: string | null;
	target_name: string | null;
}

/**
 * Audit rows for the e2e audit-browser flow, spanning the 7-day default window
 * (today / yesterday / within-window) plus older rows that only appear once the
 * window is widened (driving the window-edge "widen" affordance). "Wasp 223"
 * appears as both an actor and a target name so the combined `q` search and
 * click-to-refine can be exercised.
 */
function seededAuditEntries(): AuditEntry[] {
	const now = new Date();
	const DAY = 86_400_000;
	const HOUR = 3_600_000;
	const MIN = 60_000;

	// Anchor to local midnight so "today" entries are always in today's calendar
	// day regardless of what time the test runs, and "yesterday" is always the
	// prior day. Using wall-clock ago(N*HOUR) is fragile near midnight.
	const todayMidnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
	const atToday = (offsetMs: number) => new Date(todayMidnight + offsetMs).toISOString();
	const atYesterday = (offsetMs: number) => new Date(todayMidnight - DAY + offsetMs).toISOString();
	const atDaysAgo = (days: number, offsetMs: number) =>
		new Date(todayMidnight - days * DAY + offsetMs).toISOString();

	const base = (over: Partial<AuditEntry>): AuditEntry => ({
		id: 'x',
		occurred_at: atToday(HOUR),
		actor_account_id: 'acc1',
		actor_character_id: 1001,
		actor_character_name: 'Main Pilot',
		event_type: 'account_registered',
		details: {},
		target_type: 'account',
		target_id: 'acc1',
		target_name: 'Main Pilot',
		...over
	});

	return [
		// Today — Wasp as actor.
		base({
			id: 'a1',
			occurred_at: atToday(10 * HOUR + 30 * MIN),
			actor_character_name: 'Wasp 223',
			event_type: 'acl_member_added',
			// Self-contained: the added member's name is snapshotted into details
			// (the Details dialog reads this verbatim, no id resolution).
			details: { member_name: 'Wasp 222', member_type: 'character', permission: 'admin' },
			target_type: 'acl',
			target_id: 'acl-1',
			target_name: 'Corp ACL'
		}),
		// Today — Wasp as target name, different actor.
		base({
			id: 'a2',
			occurred_at: atToday(9 * HOUR),
			actor_character_name: 'Other Pilot',
			actor_account_id: 'acc2',
			event_type: 'map_created',
			target_type: 'map',
			target_id: 'map-1',
			target_name: 'Red Wasp Industries Map'
		}),
		// Today — security-relevant.
		base({
			id: 'a3',
			occurred_at: atToday(8 * HOUR),
			actor_account_id: null,
			actor_character_id: null,
			actor_character_name: null,
			event_type: 'blocked_login_rejected',
			target_type: 'character',
			target_id: '98765',
			target_name: null
		}),
		// Yesterday.
		base({
			id: 'b1',
			occurred_at: atYesterday(22 * HOUR),
			event_type: 'character_added',
			target_type: 'character',
			target_id: '555',
			target_name: 'Alt Pilot'
		}),
		// Three days ago (still inside 7d).
		base({
			id: 'c1',
			occurred_at: atDaysAgo(3, 12 * HOUR),
			event_type: 'api_key_created'
		}),
		// 20 days ago — outside 7d, inside 30d (only after widening).
		base({
			id: 'd1',
			occurred_at: atDaysAgo(20, 12 * HOUR),
			actor_character_name: 'Wasp 223',
			event_type: 'acl_renamed',
			target_type: 'acl',
			target_id: 'acl-9',
			target_name: 'Old Wasp ACL'
		})
	];
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
	characters: {
		eve_character_id: number;
		name: string;
		is_main: boolean;
		token_status: 'active' | 'expired' | 'owner_mismatch';
	}[];
};
const adminAccounts: AdminAccount[] = [
	{
		id: 'acc1',
		status: 'active',
		is_server_admin: true,
		created_at: '2025-01-01T00:00:00Z',
		characters: [
			{ eve_character_id: 1001, name: 'Main Pilot', is_main: true, token_status: 'active' },
			// A transferred alt, so the Characters-tab filter has something to surface.
			{ eve_character_id: 1003, name: 'Sold Alt', is_main: false, token_status: 'owner_mismatch' }
		]
	},
	{
		id: 'acc2',
		status: 'active',
		is_server_admin: false,
		created_at: '2025-02-01T00:00:00Z',
		characters: [{ eve_character_id: 2001, name: 'Promote Me', is_main: true, token_status: 'active' }]
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

// ── maps / ACLs mutable state (account session) ──────────────────────────────
// These back the maps + ACLs e2e flows: create → attach/detach, member CRUD.
let mapSeq = 1;
let aclSeq = 1;
let memberSeq = 1;

type MockAcl = { id: string; name: string; owner_account_id: string; created_at: string; updated_at: string };
type MockMember = {
	id: string;
	acl_id: string;
	member_type: string;
	eve_entity_id: number | null;
	character_id: string | null;
	name: string;
	permission: string;
	created_at: string;
	updated_at: string;
};
type MockMap = {
	id: string;
	name: string;
	slug: string;
	owner_account_id: string;
	description: string | null;
	acl_ids: string[];
	created_at: string;
	updated_at: string;
};

const maps: MockMap[] = [];
const acls: MockAcl[] = [];
const members: MockMember[] = [];

const NOW = () => new Date().toISOString();

function aclSummaries(map: MockMap) {
	return map.acl_ids
		.map((id) => acls.find((a) => a.id === id))
		.filter((a): a is MockAcl => Boolean(a))
		.map((a) => ({ id: a.id, name: a.name }));
}

function mapDto(map: MockMap) {
	return {
		id: map.id,
		name: map.name,
		slug: map.slug,
		owner_account_id: map.owner_account_id,
		description: map.description,
		acls: aclSummaries(map),
		created_at: map.created_at,
		updated_at: map.updated_at
	};
}

// A tiny entity-search corpus so the member picker has something to resolve. The
// e2e drives this — NOT real ESI.
const entityCharacters = [
	{ id: 'ent-char-1', eve_character_id: 4001, name: 'Search Pilot' }
];
const entityCorporations = [{ eve_entity_id: 5001, name: 'Search Corp' }];
const entityAlliances = [{ eve_entity_id: 6001, name: 'Search Alliance' }];

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
			const params = new URL(url, 'http://x').searchParams;
			let entries = seededAuditEntries();

			// Window → since lower bound (day-snapped is unnecessary for the mock;
			// a plain relative cutoff is enough to exercise the UI).
			const window = params.get('window') ?? '7d';
			const days = { '7d': 7, '30d': 30, '90d': 90, '365d': 365 }[window] ?? 7;
			const since = Date.now() - days * 86_400_000;
			entries = entries.filter((e) => new Date(e.occurred_at).getTime() >= since);

			// before keyset cursor (exclusive upper bound).
			const before = params.get('before');
			if (before) {
				const cursor = new Date(before).getTime();
				entries = entries.filter((e) => new Date(e.occurred_at).getTime() < cursor);
			}

			// Equality + substring axes.
			const eventType = params.get('event_type');
			if (eventType) entries = entries.filter((e) => e.event_type === eventType);
			const actor = params.get('actor');
			if (actor) entries = entries.filter((e) => e.actor_account_id === actor);
			const targetType = params.get('target_type');
			if (targetType) entries = entries.filter((e) => e.target_type === targetType);
			const targetId = params.get('target_id');
			if (targetId) entries = entries.filter((e) => e.target_id === targetId);
			const q = (params.get('q') ?? '').toLowerCase();
			if (q) {
				entries = entries.filter(
					(e) =>
						(e.actor_character_name ?? '').toLowerCase().includes(q) ||
						(e.target_name ?? '').toLowerCase().includes(q)
				);
			}

			// Newest-first, then a small page so infinite scroll + the window edge
			// are reachable in the e2e flow.
			entries.sort((a, b) => new Date(b.occurred_at).getTime() - new Date(a.occurred_at).getTime());
			const limit = Number(params.get('limit') ?? '50');
			const page = entries.slice(0, limit);
			const next_before =
				entries.length > page.length && page.length > 0
					? page[page.length - 1].occurred_at
					: null;
			json(res, 200, { data: { entries: page, next_before } });
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

	// ── maps / ACLs / entity search (account session) ──────────────────────────
	if (
		url.startsWith('/api/v1/maps') ||
		url.startsWith('/api/v1/acls') ||
		url.startsWith('/api/v1/entities/')
	) {
		if (!hasValidSession(req)) {
			unauthorised(res);
			return;
		}

		// GET /api/v1/maps
		if (method === 'GET' && url === '/api/v1/maps') {
			json(res, 200, { data: maps.map(mapDto) });
			return;
		}
		// POST /api/v1/maps
		if (method === 'POST' && url === '/api/v1/maps') {
			const body = JSON.parse((await readBody(req)) || '{}');
			if (maps.some((m) => m.slug === body.slug)) {
				json(res, 409, { error: { code: 'slug_taken', message: 'Slug already in use' } });
				return;
			}
			const map: MockMap = {
				id: `map-${mapSeq++}`,
				name: body.name,
				slug: body.slug,
				owner_account_id: 'acc1',
				description: body.description ?? null,
				acl_ids: body.acl_id ? [body.acl_id] : [],
				created_at: NOW(),
				updated_at: NOW()
			};
			maps.push(map);
			json(res, 201, { data: mapDto(map) });
			return;
		}
		// POST /api/v1/maps/{id}/acls — attach
		const attachMatch = url.match(/^\/api\/v1\/maps\/([^/]+)\/acls$/);
		if (method === 'POST' && attachMatch) {
			const map = maps.find((m) => m.id === attachMatch[1]);
			const body = JSON.parse((await readBody(req)) || '{}');
			if (!map) {
				notFound(res);
				return;
			}
			if (!map.acl_ids.includes(body.acl_id)) map.acl_ids.push(body.acl_id);
			noContent(res);
			return;
		}
		// DELETE /api/v1/maps/{id}/acls/{aclId} — detach
		const detachMatch = url.match(/^\/api\/v1\/maps\/([^/]+)\/acls\/([^/]+)$/);
		if (method === 'DELETE' && detachMatch) {
			const map = maps.find((m) => m.id === detachMatch[1]);
			if (!map) {
				notFound(res);
				return;
			}
			map.acl_ids = map.acl_ids.filter((id) => id !== detachMatch[2]);
			noContent(res);
			return;
		}
		// PATCH / DELETE /api/v1/maps/{id}
		const mapIdMatch = url.match(/^\/api\/v1\/maps\/([^/]+)$/);
		if (mapIdMatch) {
			const map = maps.find((m) => m.id === mapIdMatch[1]);
			if (!map) {
				notFound(res);
				return;
			}
			if (method === 'PATCH') {
				const body = JSON.parse((await readBody(req)) || '{}');
				map.name = body.name;
				map.slug = body.slug;
				map.description = body.description ?? null;
				map.updated_at = NOW();
				json(res, 200, { data: mapDto(map) });
				return;
			}
			if (method === 'DELETE') {
				maps.splice(maps.indexOf(map), 1);
				noContent(res);
				return;
			}
		}

		// GET /api/v1/acls
		if (method === 'GET' && url === '/api/v1/acls') {
			json(res, 200, { data: acls });
			return;
		}
		// POST /api/v1/acls
		if (method === 'POST' && url === '/api/v1/acls') {
			const body = JSON.parse((await readBody(req)) || '{}');
			const acl: MockAcl = {
				id: `acl-${aclSeq++}`,
				name: body.name,
				owner_account_id: 'acc1',
				created_at: NOW(),
				updated_at: NOW()
			};
			acls.push(acl);
			json(res, 201, { data: acl });
			return;
		}
		// GET / POST /api/v1/acls/{id}/members
		const membersMatch = url.match(/^\/api\/v1\/acls\/([^/]+)\/members$/);
		if (membersMatch) {
			const aclId = membersMatch[1];
			if (method === 'GET') {
				json(res, 200, { data: members.filter((m) => m.acl_id === aclId) });
				return;
			}
			if (method === 'POST') {
				const body = JSON.parse((await readBody(req)) || '{}');
				const member: MockMember = {
					id: `mem-${memberSeq++}`,
					acl_id: aclId,
					member_type: body.member_type,
					eve_entity_id: body.eve_entity_id ?? null,
					character_id: body.character_id ?? null,
					name: body.name || 'Member',
					permission: body.permission,
					created_at: NOW(),
					updated_at: NOW()
				};
				members.push(member);
				json(res, 201, { data: member });
				return;
			}
		}
		// PATCH / DELETE /api/v1/acls/{id}/members/{memberId}
		const memberIdMatch = url.match(/^\/api\/v1\/acls\/([^/]+)\/members\/([^/]+)$/);
		if (memberIdMatch) {
			const member = members.find((m) => m.id === memberIdMatch[2]);
			if (!member) {
				notFound(res);
				return;
			}
			if (method === 'PATCH') {
				const body = JSON.parse((await readBody(req)) || '{}');
				member.permission = body.permission;
				member.updated_at = NOW();
				json(res, 200, { data: member });
				return;
			}
			if (method === 'DELETE') {
				members.splice(members.indexOf(member), 1);
				noContent(res);
				return;
			}
		}
		// PATCH / DELETE /api/v1/acls/{id}
		const aclIdMatch = url.match(/^\/api\/v1\/acls\/([^/]+)$/);
		if (aclIdMatch) {
			const acl = acls.find((a) => a.id === aclIdMatch[1]);
			if (!acl) {
				notFound(res);
				return;
			}
			if (method === 'PATCH') {
				const body = JSON.parse((await readBody(req)) || '{}');
				acl.name = body.name;
				acl.updated_at = NOW();
				json(res, 200, { data: acl });
				return;
			}
			if (method === 'DELETE') {
				acls.splice(acls.indexOf(acl), 1);
				for (const m of maps) m.acl_ids = m.acl_ids.filter((id) => id !== acl.id);
				noContent(res);
				return;
			}
		}

		// GET /api/v1/entities/search?q=…
		if (method === 'GET' && url.startsWith('/api/v1/entities/search')) {
			const q = (new URL(url, 'http://x').searchParams.get('q') ?? '').toLowerCase();
			json(res, 200, {
				data: {
					characters: entityCharacters.filter((c) => c.name.toLowerCase().includes(q)),
					corporations: entityCorporations.filter((c) => c.name.toLowerCase().includes(q)),
					alliances: entityAlliances.filter((a) => a.name.toLowerCase().includes(q)),
					unavailable: false
				}
			});
			return;
		}

		notFound(res);
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
