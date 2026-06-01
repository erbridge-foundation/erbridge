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

	test('characters tab: search → inspect account dialog → filter by token state', async ({
		page
	}) => {
		await page.goto('/admin/characters');
		await expect(page.getByRole('heading', { name: 'CHARACTERS' })).toBeVisible();

		// Search for a character on the seeded admin account.
		await page.getByPlaceholder('search characters by name…').fill('Main');
		await page.getByRole('button', { name: 'search' }).click();

		// Inspect its account.
		const result = page.locator('li', { hasText: 'Main Pilot' });
		await expect(result).toBeVisible();
		await result.getByRole('button', { name: 'inspect account' }).click();

		// The dialog shows the whole account, including the transferred alt.
		// Scope name assertions to the table cells (the title also says "Main Pilot").
		const dialog = page.getByRole('dialog', { name: /Account for Main Pilot/ });
		await expect(dialog).toBeVisible();
		const charCells = dialog.locator('.char-name');
		await expect(charCells.filter({ hasText: 'Main Pilot' })).toBeVisible();
		await expect(charCells.filter({ hasText: 'Sold Alt' })).toBeVisible();
		await expect(dialog.getByText('transferred')).toBeVisible();

		// Filtering to "needs attention" hides the healthy main, keeps the alt.
		await dialog.getByRole('button', { name: 'needs attention' }).click();
		await expect(charCells.filter({ hasText: 'Main Pilot' })).toHaveCount(0);
		await expect(charCells.filter({ hasText: 'Sold Alt' })).toBeVisible();

		// Close the dialog.
		await dialog.getByRole('button', { name: 'close' }).click();
		await expect(page.getByRole('dialog')).toHaveCount(0);
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
