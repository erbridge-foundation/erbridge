/**
 * E2E tests for the /admin route group.
 *
 * Runs against the built SvelteKit app + the mock backend
 * (tests/e2e/mock-backend.ts). The mock treats a session cookie value of
 * "admin-session" as a server admin (is_server_admin: true) and serves
 * /api/v1/admin/*; any other non-empty value is a non-admin, for whom the
 * /admin route group 404s (the server-side load gate).
 *
 * Coverage (§9.10):
 *   - admin sees /admin and its sub-pages
 *   - non-admin gets 404 on /admin
 *   - grant → revoke flow (admins page)
 *   - block → unblock flow (blocks page)
 *   - a blocked user lands on /blocked
 */

import { test, expect, type BrowserContext } from '@playwright/test';

async function signIn(context: BrowserContext, value: string) {
	await context.addCookies([
		{
			name: 'session',
			value,
			domain: 'localhost',
			path: '/',
			httpOnly: false,
			secure: false,
			sameSite: 'Lax'
		}
	]);
}

test.describe('/admin (admin session)', () => {
	test.beforeEach(async ({ context }) => {
		await signIn(context, 'admin-session');
	});

	test('admin can open the overview and its sub-pages', async ({ page }) => {
		await page.goto('/admin');
		await expect(page).toHaveURL(/\/admin$/);
		await expect(page.getByRole('heading', { name: 'ADMIN' })).toBeVisible();

		await page.getByRole('link', { name: 'admins', exact: true }).click();
		await expect(page).toHaveURL(/\/admin\/admins$/);
		await expect(page.getByRole('heading', { name: 'SERVER ADMINS' })).toBeVisible();

		// Scope to the admin-tabs nav: a global header link "characters" also exists.
		const adminNav = page.getByRole('navigation', { name: 'Admin sections' });
		await adminNav.getByRole('link', { name: 'characters', exact: true }).click();
		await expect(page).toHaveURL(/\/admin\/characters$/);
		await expect(page.getByRole('heading', { name: 'CHARACTERS' })).toBeVisible();

		await page.getByRole('link', { name: 'blocks', exact: true }).click();
		await expect(page).toHaveURL(/\/admin\/blocks$/);
		await expect(page.getByRole('heading', { name: 'BLOCKED CHARACTERS' })).toBeVisible();

		await page.getByRole('link', { name: 'audit', exact: true }).click();
		await expect(page).toHaveURL(/\/admin\/audit$/);
		await expect(page.getByRole('heading', { name: 'AUDIT LOG' })).toBeVisible();
	});

	test('the user menu surfaces an Admin link for an admin', async ({ page }) => {
		await page.goto('/characters');
		await page.getByRole('button', { name: /Main Pilot/ }).click();
		const adminLink = page.getByRole('menuitem', { name: 'admin' });
		await expect(adminLink).toBeVisible();
		await expect(adminLink).toHaveAttribute('href', '/admin');
	});

	test('grant → revoke flow via character search', async ({ page }) => {
		await page.goto('/admin/admins');

		// Search for the promotable character.
		await page.getByPlaceholder('search characters by name…').fill('Promote');
		await page.getByRole('button', { name: 'search' }).click();

		// Promote the account that owns it.
		await page.getByRole('button', { name: 'promote' }).click();
		const promoteDialog = page.getByRole('alertdialog', { name: /Promote the account/ });
		await expect(promoteDialog).toBeVisible();
		await promoteDialog.getByRole('button', { name: 'grant admin' }).click();

		// The newly promoted account now appears in the admins table.
		await expect(page.locator('tbody').getByText('Promote Me')).toBeVisible();

		// Revoke it again.
		const row = page.locator('tr', { hasText: 'Promote Me' });
		await row.getByRole('button', { name: 'revoke' }).click();
		const revokeDialog = page.getByRole('alertdialog', { name: /Revoke admin from/ });
		await expect(revokeDialog).toBeVisible();
		await revokeDialog.getByRole('button', { name: 'revoke admin' }).click();

		await expect(page.locator('tbody').getByText('Promote Me')).toHaveCount(0);
	});

	test('characters tab: grid lists accounts, expands to token state, filters by status', async ({
		page
	}) => {
		await page.goto('/admin/characters');
		await expect(page.getByRole('heading', { name: 'CHARACTERS' })).toBeVisible();

		// The grid lists the seeded admin account, labelled by its main, with the
		// transferred alt rolled up into the Issues column while collapsed.
		const row = page.locator('tr', { hasText: 'Main Pilot' });
		await expect(row.locator('.account-cell')).toHaveText('Main Pilot');
		await expect(row.getByText('1 transferred')).toBeVisible();

		// Expanding reveals the per-character token table including the alt's state.
		await row.getByRole('button', { name: /show characters for Main Pilot/i }).click();
		const detail = page.locator('.char-table');
		await expect(detail.locator('.char-name', { hasText: 'Main Pilot' })).toBeVisible();
		await expect(detail.locator('.char-name', { hasText: 'Sold Alt' })).toBeVisible();
		await expect(detail.getByText('transferred')).toBeVisible();

		// Collapsing hides the detail table again.
		await row.getByRole('button', { name: /hide characters for Main Pilot/i }).click();
		await expect(page.locator('.char-table')).toHaveCount(0);

		// Filtering by character name surfaces the account via its alt.
		await page.getByLabel('Filter accounts by character name').fill('Sold');
		await expect(page.locator('tr', { hasText: 'Main Pilot' })).toBeVisible();

		// The "transferred" status chip keeps the flagged account.
		await page.getByLabel('Filter accounts by character name').fill('');
		await page.getByRole('button', { name: 'transferred', exact: true }).click();
		await expect(page.locator('tr.account-row', { hasText: 'Main Pilot' })).toBeVisible();
	});

	test('block via local search → unblock flow', async ({ page }) => {
		await page.goto('/admin/blocks');
		await expect(page.getByText('No blocked characters.')).toBeVisible();

		// No raw character-ID field exists any more.
		await expect(page.getByLabel('EVE character ID')).toHaveCount(0);

		// Search the local index by name and pick a result. "Promote Me" (id 2001)
		// belongs to a non-admin account in the mock, so it is not a self-block.
		await page.getByPlaceholder('search characters by name…').fill('Promote');
		await page.getByRole('button', { name: 'search' }).click();
		const result = page.locator('li', { hasText: 'Promote Me' });
		await expect(result).toBeVisible();
		await result.getByRole('button', { name: 'block' }).click();

		// Confirmation enriched with corp (from the mocked public-ESI lookup).
		const dialog = page.getByRole('alertdialog', { name: /Block Promote Me/ });
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'block character' }).click();

		// The blocked character appears in the list (mock names it "Char <id>").
		const blockedRow = page.locator('tr', { hasText: 'Char 2001' });
		await expect(blockedRow).toBeVisible();

		await blockedRow.getByRole('button', { name: 'unblock' }).click();
		const unblockDialog = page.getByRole('alertdialog', { name: /Unblock/ });
		await expect(unblockDialog).toBeVisible();
		await unblockDialog.getByRole('button', { name: 'unblock character' }).click();

		await expect(page.getByText('No blocked characters.')).toBeVisible();
	});

	test('block a never-seen pilot via ESI fallback', async ({ page }) => {
		await page.goto('/admin/blocks');

		// Local search for a name the index does not know → empty + ESI opt-in.
		await page.getByPlaceholder('search characters by name…').fill('Esi Only');
		await page.getByRole('button', { name: 'search' }).click();
		await expect(page.getByText('No local characters match.')).toBeVisible();

		await page.getByRole('button', { name: 'Not found? Search ESI' }).click();
		const esiResult = page.locator('li', { hasText: 'Esi Only Pilot' });
		await expect(esiResult).toBeVisible();
		await esiResult.getByRole('button', { name: 'block' }).click();

		const dialog = page.getByRole('alertdialog', { name: /Block Esi Only Pilot/ });
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'block character' }).click();

		await expect(page.locator('tr', { hasText: 'Char 90000123' })).toBeVisible();

		// Clean up so the suite's shared mock state is reset for other tests.
		await page
			.locator('tr', { hasText: 'Char 90000123' })
			.getByRole('button', { name: 'unblock' })
			.click();
		await page
			.getByRole('alertdialog', { name: /Unblock/ })
			.getByRole('button', { name: 'unblock character' })
			.click();
		await expect(page.getByText('No blocked characters.')).toBeVisible();
	});
});

test.describe('/admin/audit browser', () => {
	test.beforeEach(async ({ context }) => {
		await signIn(context, 'admin-session');
	});

	test('browse default view groups rows under day headers within the 7-day window', async ({
		page
	}) => {
		await page.goto('/admin/audit');
		await expect(page.getByRole('heading', { name: 'AUDIT LOG' })).toBeVisible();

		// Day-group headers orient the stream.
		await expect(page.getByRole('columnheader', { name: 'Today' })).toBeVisible();
		await expect(page.getByRole('columnheader', { name: 'Yesterday' })).toBeVisible();

		// A within-window row is present; the 20-days-ago row is not (outside 7d).
		await expect(page.getByText('Corp ACL')).toBeVisible();
		await expect(page.getByText('Old Wasp ACL')).toHaveCount(0);
	});

	test('directed search → result → click-to-refine → clear', async ({ page }) => {
		await page.goto('/admin/audit');

		// Directed: search "wasp" finds both the actor-side and target-side rows.
		await page.getByLabel('Search').fill('wasp');
		await page.getByLabel('Search').press('Enter');
		await expect(page).toHaveURL(/q=wasp/);
		await expect(page.getByText('Corp ACL')).toBeVisible();
		await expect(page.getByText('Red Wasp Industries Map')).toBeVisible();

		// A search chip appears.
		await expect(page.getByText('Search: wasp')).toBeVisible();

		// Refine: click the ACL target cell → target filter pins that entity.
		await page.getByRole('button', { name: 'Corp ACL' }).click();
		await expect(page).toHaveURL(/target_type=acl/);
		await expect(page).toHaveURL(/target_id=acl-1/);
		await expect(page.getByText('Target: acl acl-1')).toBeVisible();
		// The unrelated map row is gone now.
		await expect(page.getByText('Red Wasp Industries Map')).toHaveCount(0);

		// Clear all → back to the default browse view.
		await page.getByRole('link', { name: 'clear all' }).click();
		await expect(page).toHaveURL(/\/admin\/audit$/);
		await expect(page.getByText('Red Wasp Industries Map')).toBeVisible();
	});

	test('window edge offers widening rather than silent expansion', async ({ page }) => {
		await page.goto('/admin/audit');

		// At the bottom of the 7-day window, the edge affordance is shown.
		const widen = page.getByRole('button', { name: 'Widen to Last 30 days' });
		await expect(widen).toBeVisible();

		// The older (20-day) row only appears after widening.
		await expect(page.getByText('Old Wasp ACL')).toHaveCount(0);
		await widen.click();
		await expect(page).toHaveURL(/window=30d/);
		await expect(page.getByText('Old Wasp ACL')).toBeVisible();
	});

	test('a row Details dialog shows the snapshotted fields and closes', async ({ page }) => {
		await page.goto('/admin/audit');

		// The acl_member_added row (target "Corp ACL") carries the added member's
		// name in details. Open its Details dialog.
		const dialog = page.getByRole('dialog', { name: 'Event details' });
		await expect(dialog).toBeHidden();

		await page.getByRole('button', { name: 'View details' }).first().click();
		await expect(dialog).toBeVisible();

		// The snapshotted member name is rendered verbatim — answering "who was
		// added" without leaving the page or any id resolution.
		await expect(dialog.getByText('member_name')).toBeVisible();
		await expect(dialog.getByText('Wasp 222')).toBeVisible();

		// Dismiss; the dialog closes and the audit list is unchanged.
		await dialog.getByRole('button', { name: 'Close' }).click();
		await expect(dialog).toBeHidden();
		await expect(page.getByText('Corp ACL')).toBeVisible();
	});
});

test.describe('/admin (non-admin session)', () => {
	test('a non-admin gets a 404 and no Admin link', async ({ page, context }) => {
		await signIn(context, 'plain-session');

		const response = await page.goto('/admin');
		expect(response?.status()).toBe(404);

		// The user menu shows no Admin affordance for a non-admin.
		await page.goto('/characters');
		await page.getByRole('button', { name: /Main Pilot/ }).click();
		await expect(page.getByRole('menuitem', { name: 'admin' })).toHaveCount(0);
	});
});

test.describe('/blocked landing', () => {
	test('renders the informational page (public, no session needed)', async ({ page }) => {
		await page.goto('/blocked');
		await expect(page).toHaveURL(/\/blocked$/);
		await expect(page.getByRole('heading', { name: 'Access blocked' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'back to login' })).toHaveAttribute(
			'href',
			'/login'
		);
	});
});
