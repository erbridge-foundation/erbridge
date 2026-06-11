/**
 * E2E test for the logout control (harden-auth-flow §4).
 *
 * Logout is now a state-changing POST so it cannot be triggered by a cross-site
 * top-level navigation or a browser prefetch under SameSite=Lax. The user-menu
 * control must therefore be a `<form method="POST" action="/auth/logout">`
 * submit button, NOT a GET `<a href>`.
 *
 * The `/auth/*` routes are served by the backend at the edge, not by the
 * SvelteKit server, so they are not reachable in the e2e harness (no Traefik,
 * no mock route on the preview server). This test asserts the rendered control
 * is wired as a POST form rather than following the navigation.
 */

import { test, expect } from '@playwright/test';

test.describe('logout control', () => {
	test.beforeEach(async ({ page, context }) => {
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

	test('log out is a POST form to /auth/logout, not a GET link', async ({ page }) => {
		// Open the user menu. The chip button's accessible name is the main
		// character's name (from the mock backend) and it controls #user-menu.
		await page.locator('button[aria-controls="user-menu"]').click();

		const logout = page.getByRole('menuitem', { name: 'log out' });
		await expect(logout).toBeVisible();

		// The control is a submit button inside a POST form targeting the backend.
		const tagName = await logout.evaluate((el) => el.tagName);
		expect(tagName).toBe('BUTTON');
		await expect(logout).toHaveAttribute('type', 'submit');

		const form = page.locator('form[action="/auth/logout"]');
		await expect(form).toHaveAttribute('method', /post/i);
	});
});
