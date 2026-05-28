/**
 * E2E tests for the /account page confirm-dialog wiring (Danger zone tab).
 *
 * Mirrors the delete-account modal coverage that previously lived in
 * characters-confirm-dialog.spec.ts; the action moved to /account in the
 * add-account-page-and-api-keys change. Drives the built SvelteKit app
 * against the same mock backend (tests/e2e/mock-backend.ts), which serves
 * an empty /api/v1/keys list so /account renders without populated rows.
 */

import { test, expect } from '@playwright/test';

test.describe('/account confirm-dialog', () => {
	test.beforeEach(async ({ page, context }) => {
		// The mock backend returns 401 for authenticated endpoints unless a
		// non-empty session cookie is present.
		await context.addCookies([
			{
				name: 'session',
				value: 'test-session-token',
				domain: 'localhost',
				path: '/',
				httpOnly: false,
				secure: false,
				sameSite: 'Lax'
			}
		]);

		await page.goto('/account');
		await expect(page).toHaveURL(/\/account/);
	});

	test('clicking delete account opens the account-deletion modal', async ({ page }) => {
		// Switch to the Danger zone tab.
		await page.getByRole('tab', { name: 'Danger zone' }).click();

		await page.getByRole('button', { name: 'delete account' }).click();

		const dialog = page.getByRole('alertdialog');
		await expect(dialog).toBeVisible();
		await expect(dialog).toContainText('Delete account?');
		await expect(dialog).toContainText('30 days');
	});

	test('cancelling the delete-account modal does not submit', async ({ page }) => {
		await page.getByRole('tab', { name: 'Danger zone' }).click();

		await page.getByRole('button', { name: 'delete account' }).click();
		await expect(page.getByRole('alertdialog')).toBeVisible();

		await page.getByRole('button', { name: 'cancel' }).click();
		await expect(page.getByRole('alertdialog')).toHaveCount(0);

		// Still on /account (no redirect to /login).
		await expect(page).toHaveURL(/\/account/);
	});
});
