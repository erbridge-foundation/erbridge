/**
 * E2E for the /acls surface: create an ACL → open its detail → add a member via
 * the entity-search picker → change its permission → remove it. Runs against the
 * built app + mock backend (which serves /api/v1/entities/search from a small
 * in-memory corpus — NOT real ESI).
 *
 * Asserts the destructive-action wiring (remove member, delete ACL go through
 * the ConfirmDialog).
 */

import { test, expect, type BrowserContext } from '@playwright/test';

async function signIn(context: BrowserContext, value = 'acls-session') {
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

test.describe('/acls', () => {
	test.beforeEach(async ({ context }) => {
		await signIn(context);
	});

	test('create ACL, add a member via the picker, update permission, remove', async ({ page }) => {
		await page.goto('/acls');

		// Create the ACL via the dialog.
		await page.getByRole('button', { name: 'create ACL' }).click();
		const createDialog = page.getByRole('dialog');
		await expect(createDialog).toBeVisible();
		await createDialog.getByRole('textbox', { name: 'Name' }).fill('Fleet Cmd');
		await createDialog.getByRole('button', { name: 'create ACL' }).click();
		const aclLink = page.getByRole('link', { name: 'Fleet Cmd' });
		await expect(aclLink).toBeVisible();

		// Open its UUID-keyed detail.
		await aclLink.click();
		await expect(page.getByRole('heading', { name: 'Fleet Cmd' })).toBeVisible();
		await expect(page.getByText('No members yet.')).toBeVisible();

		// Search the entity-search picker. Pressing Enter in the field initiates
		// the search (no need to click the button).
		await page.getByRole('searchbox').fill('Search Pilot');
		await page.getByRole('searchbox').press('Enter');
		const result = page.locator('li', { hasText: 'Search Pilot' });
		await expect(result).toBeVisible();

		// The role select + add button are inline in the result row — pick manage
		// and add directly, no select-then-scroll.
		await result.getByRole('combobox').selectOption({ label: 'Manage' });
		await result.getByRole('button', { name: 'add member' }).click();

		// The member appears in the table.
		const memberRow = page.locator('tr', { hasText: 'Search Pilot' });
		await expect(memberRow).toBeVisible();
		await expect(memberRow.getByRole('combobox')).toHaveValue('manage');

		// Change its permission inline (auto-submits on change).
		await memberRow.getByRole('combobox').selectOption({ label: 'Read' });
		await expect(page.locator('tr', { hasText: 'Search Pilot' }).getByRole('combobox')).toHaveValue(
			'read'
		);

		// Remove the member — confirmation dialog wiring.
		await page
			.locator('tr', { hasText: 'Search Pilot' })
			.getByRole('button', { name: 'remove' })
			.click();
		const removeDialog = page.getByRole('alertdialog', { name: /Remove Search Pilot/ });
		await expect(removeDialog).toBeVisible();
		await removeDialog.getByRole('button', { name: 'remove member' }).click();
		await expect(page.getByText('No members yet.')).toBeVisible();

		// Delete the ACL (destructive-action wiring).
		await page.goto('/acls');
		const row = page.locator('li', { hasText: 'Fleet Cmd' });
		await row.getByRole('button', { name: 'delete' }).click();
		const deleteDialog = page.getByRole('alertdialog', { name: /Delete Fleet Cmd/ });
		await expect(deleteDialog).toBeVisible();
		await deleteDialog.getByRole('button', { name: 'delete ACL' }).click();
		await expect(page.getByRole('link', { name: 'Fleet Cmd' })).toHaveCount(0);
	});
});
