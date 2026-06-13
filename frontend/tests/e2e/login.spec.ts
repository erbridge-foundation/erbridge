import { test, expect } from '@playwright/test';

test('login page renders with EVE SSO button', async ({ page }) => {
	await page.goto('/login');
	await expect(page).toHaveTitle(/E-R Bridge.*Login/);
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

test('Maximize accessibility applies the preset to <html>', async ({ page }) => {
	await page.goto('/login');
	const html = page.locator('html');
	// Baseline: no overrides set.
	await expect(html).not.toHaveAttribute('data-high-contrast', 'on');

	const toggle = page.getByRole('checkbox', { name: /maximize accessibility/i });
	await toggle.check();

	// text_size: large → 125% root font-size; the four tri-state/toggle keys → data-* attrs.
	await expect(html).toHaveAttribute('data-high-contrast', 'on');
	await expect(html).toHaveAttribute('data-reduce-motion', 'on');
	await expect(html).toHaveAttribute('data-large-targets', 'on');
	await expect(html).toHaveAttribute('data-dyslexia-font', 'on');
	await expect(html).toHaveCSS('font-size', '20px'); // 125% of the 16px default

	// The on-state disclosure appears.
	await expect(page.getByText(/applied to this screen/i)).toBeVisible();

	// Reversible: unchecking clears the overrides.
	await toggle.uncheck();
	await expect(html).not.toHaveAttribute('data-high-contrast', 'on');
	await expect(html).not.toHaveAttribute('data-dyslexia-font', 'on');
});

test('selecting a language re-renders the login card in that language', async ({ page }) => {
	await page.goto('/login');
	// Default disclaimer is English.
	await expect(page.getByText(/Authentication is handled by EVE Online/i)).toBeVisible();

	await page.getByLabel(/Language/i).selectOption('de');

	// The locale bridge reloads the page; the card re-renders in German.
	await expect(page.getByText(/Die Authentifizierung erfolgt über EVE Online/i)).toBeVisible();
	await expect(page.getByText(/Barrierefreiheit maximieren/i)).toBeVisible();
});
