import { test, expect } from '@playwright/test';

test('login page renders with EVE SSO button', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByAltText(/LOG IN with EVE Online/i)).toBeVisible();
	await expect(page.getByText(/Wormhole Mapper/i)).toBeVisible();
	await expect(page.getByText(/Authentication is handled by EVE Online/i)).toBeVisible();
});

test('login page has no global nav', async ({ page }) => {
	await page.goto('/login');
	await expect(page.locator('header.global-nav')).toHaveCount(0);
});

test('SSO link points at backend /auth/login', async ({ page }) => {
	await page.goto('/login');
	const sso = page.getByRole('link', { name: /LOG IN with EVE Online/i });
	await expect(sso).toHaveAttribute('href', '/auth/login');
});
