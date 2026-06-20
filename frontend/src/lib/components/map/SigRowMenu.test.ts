import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import SigRowMenu from './SigRowMenu.svelte';

// globals: false in vitest.config → register cleanup explicitly.
afterEach(cleanup);

function renderMenu(overrides: Record<string, unknown> = {}) {
	const props = {
		x: 10,
		y: 20,
		onEdit: vi.fn(),
		onDelete: vi.fn(),
		onClose: vi.fn(),
		...overrides
	};
	return { props, ...render(SigRowMenu, { props }) };
}

describe('SigRowMenu', () => {
	it('renders Edit and Delete items', () => {
		renderMenu();
		expect(screen.getByRole('menuitem', { name: 'Edit' })).toBeInTheDocument();
		expect(screen.getByRole('menuitem', { name: 'Delete' })).toBeInTheDocument();
	});

	it('fires onEdit / onDelete when the items are clicked', async () => {
		const { props } = renderMenu();
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
		expect(props.onEdit).toHaveBeenCalledOnce();
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Delete' }));
		expect(props.onDelete).toHaveBeenCalledOnce();
	});

	it('closes on Escape', async () => {
		const { props } = renderMenu();
		await fireEvent.keyDown(document, { key: 'Escape' });
		expect(props.onClose).toHaveBeenCalledOnce();
	});

	it('closes on an outside pointer-down', async () => {
		const { props } = renderMenu();
		await fireEvent.pointerDown(document.body);
		expect(props.onClose).toHaveBeenCalledOnce();
	});
});
