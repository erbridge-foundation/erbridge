import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import UserMenu from './UserMenu.svelte';

afterEach(() => cleanup());

describe('UserMenu', () => {
	it('renders preferences as an enabled link to /preferences', () => {
		render(UserMenu, { props: { onclose: () => {} } });
		const link = screen.getByRole('menuitem', { name: 'preferences' });
		expect(link.tagName).toBe('A');
		expect(link).toHaveAttribute('href', '/preferences');
		expect(link).not.toHaveAttribute('aria-disabled');
	});

	it('renders account as an enabled link to /account', () => {
		render(UserMenu, { props: { onclose: () => {} } });
		const link = screen.getByRole('menuitem', { name: 'account' });
		expect(link.tagName).toBe('A');
		expect(link).toHaveAttribute('href', '/account');
		expect(link).not.toHaveAttribute('aria-disabled');
	});

	it('orders items preferences, account, about, then log out', () => {
		render(UserMenu, { props: { onclose: () => {} } });
		const labels = screen.getAllByRole('menuitem').map((el) => el.textContent?.trim());
		expect(labels).toEqual(['preferences', 'account', 'about', 'log out']);
	});

	it('calls onclose when a navigation item is clicked (so the menu closes)', async () => {
		const onclose = vi.fn();
		render(UserMenu, { props: { onclose } });
		await fireEvent.click(screen.getByRole('menuitem', { name: 'preferences' }));
		expect(onclose).toHaveBeenCalledTimes(1);
	});

	it('does NOT render the admin link for a non-admin (isAdmin omitted)', () => {
		render(UserMenu, { props: { onclose: () => {} } });
		expect(screen.queryByRole('menuitem', { name: 'admin' })).toBeNull();
	});

	it('renders the admin link to /admin only when isAdmin is true', () => {
		render(UserMenu, { props: { onclose: () => {}, isAdmin: true } });
		const link = screen.getByRole('menuitem', { name: 'admin' });
		expect(link.tagName).toBe('A');
		expect(link).toHaveAttribute('href', '/admin');
	});
});
