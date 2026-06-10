/**
 * E2E for the /maps surface: create a map via the dialog (with a default ACL) →
 * open its settings → attach another ACL → detach → delete. Also checks the map
 * name link opens the canvas placeholder, and the edit button opens settings.
 *
 * Runs against the built SvelteKit app + the mock backend, which holds
 * maps/ACLs/members in memory for the run. Asserts the destructive-action
 * wiring (delete map, detach ACL go through the ConfirmDialog).
 */

import { test, expect, type BrowserContext } from '@playwright/test';

async function signIn(context: BrowserContext, value = 'maps-session') {
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

test.describe('/maps', () => {
	test.beforeEach(async ({ context }) => {
		await signIn(context);
	});

	test('create with default ACL, settings attach/detach, canvas link, delete', async ({ page }) => {
		// Seed a second ACL (via the create dialog) so the settings attach control
		// has an extra option.
		await page.goto('/acls');
		await page.getByRole('button', { name: 'create ACL' }).click();
		const aclDialog = page.getByRole('dialog');
		await aclDialog.getByRole('textbox', { name: 'Name' }).fill('Extra ACL');
		await aclDialog.getByRole('button', { name: 'create ACL' }).click();
		await expect(page.getByRole('link', { name: 'Extra ACL' })).toBeVisible();

		// Create a map via the dialog, opting into a default ACL.
		await page.goto('/maps');
		await page.getByRole('button', { name: 'create map' }).click();
		const dialog = page.getByRole('dialog');
		await expect(dialog).toBeVisible();
		await dialog.getByRole('textbox', { name: 'Name' }).fill('Delve Ops');
		await dialog.getByRole('textbox', { name: 'Slug' }).fill('delve-ops');
		await dialog.getByRole('checkbox', { name: /Create a default ACL/ }).check();
		await dialog.getByRole('button', { name: 'create map' }).click();

		const mapLink = page.getByRole('link', { name: 'Delve Ops' });
		await expect(mapLink).toBeVisible();
		// The default ACL (named after the map) shows in the row's ACL summary.
		await expect(page.getByText(/ACLs: .*Delve Ops/)).toBeVisible();

		// The name link opens the canvas placeholder (no chrome — name is in the
		// browser tab title, settings is reached from the list).
		await mapLink.click();
		await expect(page).toHaveURL(/\/maps\/delve-ops$/);
		await expect(page.getByText('Map canvas coming soon.')).toBeVisible();

		// Settings is reached from the list's edit control.
		await page.goto('/maps');
		await page
			.locator('li', { hasText: 'Delve Ops' })
			.getByRole('link', { name: 'edit' })
			.click();
		await expect(page).toHaveURL(/\/maps\/delve-ops\/settings$/);
		await expect(page.getByRole('heading', { name: 'MAP SETTINGS' })).toBeVisible();
		// The default ACL is already attached.
		await expect(page.getByRole('link', { name: 'Delve Ops' })).toBeVisible();

		// Attach the extra ACL.
		await page.getByLabel('ACL to attach').selectOption({ label: 'Extra ACL' });
		await page.getByRole('button', { name: 'attach', exact: true }).click();
		await expect(page.getByRole('link', { name: 'Extra ACL' })).toBeVisible();

		// Detach it — confirmation dialog wiring.
		const detachRow = page.locator('li', { hasText: 'Extra ACL' });
		await detachRow.getByRole('button', { name: 'detach' }).click();
		const detachDialog = page.getByRole('alertdialog', { name: /Detach Extra ACL/ });
		await expect(detachDialog).toBeVisible();
		await detachDialog.getByRole('button', { name: 'detach ACL' }).click();
		await expect(page.getByRole('link', { name: 'Extra ACL' })).toHaveCount(0);

		// Edit the map name via the proper settings form (textarea description).
		await page.getByLabel('Name').fill('Delve Ops Renamed');
		await page.getByRole('button', { name: 'save' }).click();

		// Delete the map (destructive-action wiring) from the list. The edit button
		// on the list row links to settings (verified by the URL above).
		await page.goto('/maps');
		const row = page.locator('li', { hasText: 'Delve Ops' });
		await row.getByRole('button', { name: 'delete' }).click();
		const deleteDialog = page.getByRole('alertdialog', { name: /Delete Delve Ops/ });
		await expect(deleteDialog).toBeVisible();
		await deleteDialog.getByRole('button', { name: 'delete map' }).click();
		await expect(page.getByRole('link', { name: /Delve Ops/ })).toHaveCount(0);
	});

	test('an unknown slug 404s', async ({ page }) => {
		const response = await page.goto('/maps/no-such-map');
		expect(response?.status()).toBe(404);
	});
});
