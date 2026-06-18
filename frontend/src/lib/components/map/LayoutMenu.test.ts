import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import LayoutMenu from './LayoutMenu.svelte';

afterEach(cleanup);

function renderMenu(overrides: Record<string, unknown> = {}) {
	const onSelect = vi.fn();
	const onApply = vi.fn();
	const utils = render(LayoutMenu, {
		props: { layoutDir: 'LR' as const, onSelect, onApply, ...overrides }
	});
	return { onSelect, onApply, ...utils };
}

describe('LayoutMenu (tab-bar split-button)', () => {
	it("the action button reflects the current style and applies it on click", async () => {
		const { onApply } = renderMenu({ layoutDir: 'TB' });
		const apply = screen.getByRole('button', { name: 'Apply layout: Top → bottom' });
		await fireEvent.click(apply);
		expect(onApply).toHaveBeenCalledOnce();
	});

	it('the caret opens a dropdown of the four styles', async () => {
		renderMenu();
		expect(screen.queryByRole('menu')).toBeNull();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose layout style' }));
		expect(screen.getByRole('menu')).toBeInTheDocument();
		expect(screen.getByRole('menuitemradio', { name: 'Left → right' })).toBeInTheDocument();
		expect(screen.getByRole('menuitemradio', { name: 'Bottom → top' })).toBeInTheDocument();
	});

	it('marks the active style as checked in the dropdown', async () => {
		renderMenu({ layoutDir: 'RL' });
		await fireEvent.click(screen.getByRole('button', { name: 'Choose layout style' }));
		expect(screen.getByRole('menuitemradio', { name: 'Right → left' })).toHaveAttribute(
			'aria-checked',
			'true'
		);
		expect(screen.getByRole('menuitemradio', { name: 'Left → right' })).toHaveAttribute(
			'aria-checked',
			'false'
		);
	});

	it('picking a style fires onSelect and closes the dropdown', async () => {
		const { onSelect } = renderMenu();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose layout style' }));
		await fireEvent.click(screen.getByRole('menuitemradio', { name: 'Bottom → top' }));
		expect(onSelect).toHaveBeenCalledWith('BT');
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('is inert when disabled', async () => {
		const { onApply } = renderMenu({ disabled: true });
		const apply = screen.getByRole('button', { name: 'Apply layout: Left → right' });
		expect(apply).toBeDisabled();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose layout style' }));
		expect(screen.queryByRole('menu')).toBeNull();
	});
});
