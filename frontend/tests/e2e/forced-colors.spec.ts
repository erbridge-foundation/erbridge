/**
 * E2E tests for forced-colors (Windows High Contrast) support — see openspec
 * accessibility-preferences. These assert the outcomes jsdom/Vitest cannot
 * emulate: under `forced-colors: active` keyboard focus stays visible on
 * controls that use the `outline: none; border-color: var(--sky)` pattern, and
 * colour-encoded status signals keep `forced-color-adjust: none` so their
 * semantic colour survives the OS flatten.
 *
 * Driven against /characters (authed via the mock backend) because it shows
 * all three signals at once: the search input (focus pattern), the connected
 * status icon in the nav, and per-character token-status chips.
 *
 * forced-colors emulation is Chromium-only; the suite is skipped elsewhere.
 */

import { test, expect } from '@playwright/test';

test.describe('forced-colors: active', () => {
	test.beforeEach(async ({ page, context, browserName }) => {
		test.skip(browserName !== 'chromium', 'forced-colors emulation is Chromium-only');

		// The mock backend returns 401 for /api/v1/me without a non-empty session
		// cookie; add one so /characters renders authed (matching the other suites).
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

		await page.emulateMedia({ forcedColors: 'active' });
		await page.goto('/characters');
		await expect(page).toHaveURL(/\/characters/);
	});

	test('keyboard focus on the search input shows a visible outline', async ({ page }) => {
		const search = page.getByRole('searchbox');
		// Text inputs match :focus-visible even on programmatic focus, so the
		// restored keyboard-focus outline applies here.
		await search.focus();
		await expect(search).toBeFocused();

		// The everywhere focus pattern is `outline: none; border-color: var(--sky)`,
		// which collapses under forced-colors. The @media block restores a real
		// outline on :focus-visible, so a focused control must have a non-`none`
		// computed outline-style.
		const outlineStyle = await search.evaluate(
			(el) => getComputedStyle(el).outlineStyle
		);
		expect(outlineStyle).not.toBe('none');
	});

	test('the connected status icon preserves its colour signal', async ({ page }) => {
		// The nav connection indicator is now a StatusIcon glyph. Its shape carries
		// the signal under forced-colors, and forced-color-adjust: none keeps the
		// semantic token colour as a redundant channel.
		const icon = page.locator('header .status-icon').first();
		await expect(icon).toBeAttached();

		const adjust = await icon.evaluate(
			(el) => getComputedStyle(el).forcedColorAdjust
		);
		expect(adjust).toBe('none');
	});
});
