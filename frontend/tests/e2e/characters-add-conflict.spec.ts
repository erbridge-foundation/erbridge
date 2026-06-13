/**
 * E2E tests for the add-character bound-elsewhere conflict notice on /characters.
 *
 * The backend's add-character SSO callback redirects here with
 * `?add_conflict=bound_elsewhere` when the presented character is already linked
 * to another account. The /characters page renders a dismissible localised
 * notice and strips the flag from the URL (replaceState) so a reload does not
 * re-show it. These tests drive the built SvelteKit app against the mock backend
 * (tests/e2e/mock-backend.ts).
 */

import { test, expect } from '@playwright/test';

test.describe('/characters add-conflict notice', () => {
	test.beforeEach(async ({ context }) => {
		// The mock backend returns 401 for /api/v1/me unless a non-empty session
		// cookie is present.
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
	});

	test('shows the notice and strips the flag from the URL', async ({ page }) => {
		await page.goto('/characters?add_conflict=bound_elsewhere');

		const notice = page.getByRole('alert');
		await expect(notice).toBeVisible();
		await expect(notice).toContainText(/already linked to another account/i);

		// The flag is removed from the URL after rendering (replaceState), so a
		// reload would not re-show the notice.
		await expect(page).toHaveURL(/\/characters$/);
	});

	test('the notice is dismissible', async ({ page }) => {
		await page.goto('/characters?add_conflict=bound_elsewhere');
		await expect(page.getByRole('alert')).toBeVisible();

		await page.getByRole('button', { name: /dismiss notice/i }).click();
		await expect(page.getByRole('alert')).toHaveCount(0);
	});

	test('no notice without the flag', async ({ page }) => {
		await page.goto('/characters');
		await expect(page).toHaveURL(/\/characters$/);
		await expect(page.getByRole('alert')).toHaveCount(0);
	});

	test('following the add-character redirect lands on the notice', async ({ page }) => {
		// The mock backend's /auth/characters/add 303-redirects to the conflict
		// URL, mirroring the backend's bound-elsewhere outcome. Navigating it in a
		// new tab against the backend origin verifies the redirect contract; the
		// notice itself is rendered by the SvelteKit app at its own origin, so we
		// assert on the produced Location rather than a cross-origin navigation.
		const res = await page.request.get('http://127.0.0.1:9100/auth/characters/add?return_to=/characters', {
			maxRedirects: 0
		});
		expect(res.status()).toBe(303);
		expect(res.headers()['location']).toBe('/characters?add_conflict=bound_elsewhere');
	});
});
