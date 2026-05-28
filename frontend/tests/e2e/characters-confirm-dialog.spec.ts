/**
 * E2E tests for the /characters page confirm-dialog wiring.
 *
 * These tests spin up the built SvelteKit app against a lightweight mock
 * backend (started in globalSetup via tests/e2e/mock-backend.ts) that
 * serves canned /api/v1/me data and accepts DELETE requests. The mock
 * listens on http://127.0.0.1:9100 — matching the BACKEND_INTERNAL_URL used
 * by the playwright.config.ts webServer command.
 *
 * Manual verification steps (deferred from §6 — cannot be automated here):
 *   §6.2  keyboard-only flow (Tab/Shift+Tab, Enter, Escape)
 *   §6.5  prefers-reduced-motion: no visible transition
 *   §6.6  screen-reader announces title and body
 */

import { test, expect } from '@playwright/test';

// The mock backend seeds two characters; one is marked as main.
const MAIN_CHARACTER_NAME = 'Main Pilot';
const ALT_CHARACTER_NAME = 'Jita Trader';

test.describe('/characters confirm-dialog', () => {
	test.beforeEach(async ({ page, context }) => {
		// The mock backend (tests/e2e/mock-backend.ts) returns 401 for /api/v1/me
		// unless a non-empty session cookie is present. Add one for this suite.
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

		await page.goto('/characters');
		// Verify the page loaded (not redirected to /login due to missing auth).
		await expect(page).toHaveURL(/\/characters/);
	});

	test('clicking remove opens the modal with the character name in the title', async ({
		page
	}) => {
		const removeBtn = page
			.locator('article', { hasText: ALT_CHARACTER_NAME })
			.getByRole('button', { name: 'remove' });

		await removeBtn.click();

		const dialog = page.getByRole('alertdialog');
		await expect(dialog).toBeVisible();
		await expect(dialog).toContainText(`Remove ${ALT_CHARACTER_NAME}?`);
	});

	test('clicking cancel closes the modal without submitting', async ({ page }) => {
		const removeBtn = page
			.locator('article', { hasText: ALT_CHARACTER_NAME })
			.getByRole('button', { name: 'remove' });

		await removeBtn.click();
		await expect(page.getByRole('alertdialog')).toBeVisible();

		// Cancel
		await page.getByRole('button', { name: 'cancel' }).click();
		await expect(page.getByRole('alertdialog')).toHaveCount(0);

		// The character card should still be present.
		await expect(page.locator('article', { hasText: ALT_CHARACTER_NAME })).toBeVisible();
	});

	test('clicking the destructive button submits the remove form', async ({ page }) => {
		const removeBtn = page
			.locator('article', { hasText: ALT_CHARACTER_NAME })
			.getByRole('button', { name: 'remove' });

		await removeBtn.click();
		await expect(page.getByRole('alertdialog')).toBeVisible();

		// Confirm — the mock backend returns 204 and the page re-renders without
		// the character (SvelteKit's use:enhance invalidates the load).
		await page.getByRole('button', { name: 'remove character' }).click();
		await expect(page.getByRole('alertdialog')).toHaveCount(0);
	});

	test('clicking outside the dialog (backdrop) closes it without submitting', async ({
		page
	}) => {
		const removeBtn = page
			.locator('article', { hasText: ALT_CHARACTER_NAME })
			.getByRole('button', { name: 'remove' });

		await removeBtn.click();
		await expect(page.getByRole('alertdialog')).toBeVisible();

		// Click at a point guaranteed to be on the backdrop (top-left corner of viewport).
		await page.mouse.click(10, 10);
		await expect(page.getByRole('alertdialog')).toHaveCount(0);

		// Character still present (no submission).
		await expect(page.locator('article', { hasText: ALT_CHARACTER_NAME })).toBeVisible();
	});

	// The delete-account button moved to /account (Danger zone tab) in the
	// add-account-page-and-api-keys change. Its modal coverage lives in
	// account-confirm-dialog.spec.ts.
});
