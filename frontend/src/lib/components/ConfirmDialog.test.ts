import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';
import Harness from './ConfirmDialog.test.svelte';

// matchMedia is not implemented in jsdom; install a default stub. Individual
// tests that care about prefers-reduced-motion override this via vi.spyOn.
function installMatchMediaStub(matches = false) {
	Object.defineProperty(window, 'matchMedia', {
		writable: true,
		configurable: true,
		value: vi.fn().mockImplementation((query: string) => ({
			matches,
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: vi.fn(),
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn()
		}))
	});
}

describe('ConfirmDialog', () => {
	beforeEach(() => {
		installMatchMediaStub(false);
	});

	afterEach(() => {
		cleanup();
	});

	it('does not render when open is false', () => {
		render(Harness, {
			props: { open: false, onCancel: () => {}, onConfirm: () => {} }
		});

		expect(screen.queryByRole('alertdialog')).toBeNull();
	});

	it('renders title, body, and confirm label from snippets when open', () => {
		render(Harness, {
			props: {
				open: true,
				onCancel: () => {},
				onConfirm: () => {},
				titleText: 'Remove Jita Trader?',
				bodyText: 'Stored EVE SSO tokens for this character will be removed.',
				confirmLabelText: 'remove character'
			}
		});

		expect(screen.getByText('Remove Jita Trader?')).toBeInTheDocument();
		expect(screen.getByText(/Stored EVE SSO tokens/)).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'remove character' })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'cancel' })).toBeInTheDocument();
	});

	it('has role=alertdialog, aria-modal, aria-labelledby, aria-describedby', () => {
		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm: () => {} }
		});

		const dialog = screen.getByRole('alertdialog');
		expect(dialog).toHaveAttribute('aria-modal', 'true');

		const labelledById = dialog.getAttribute('aria-labelledby');
		const describedById = dialog.getAttribute('aria-describedby');
		expect(labelledById).toBeTruthy();
		expect(describedById).toBeTruthy();
		expect(document.getElementById(labelledById as string)).toHaveTextContent('Delete thing?');
		expect(document.getElementById(describedById as string)).toHaveTextContent(
			'This will permanently remove the thing.'
		);
	});

	it('default-focuses the cancel button on open', async () => {
		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm: () => {} }
		});

		// Focus is moved in a microtask after mount; flush it.
		await tick();
		await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));

		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'cancel' }));
	});

	it('calls onConfirm when the destructive button is clicked', async () => {
		const onConfirm = vi.fn();
		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm }
		});

		await fireEvent.click(screen.getByRole('button', { name: 'delete thing' }));
		expect(onConfirm).toHaveBeenCalledTimes(1);
	});

	it('calls onCancel when the cancel button is clicked', async () => {
		const onCancel = vi.fn();
		render(Harness, {
			props: { open: true, onCancel, onConfirm: () => {} }
		});

		await fireEvent.click(screen.getByRole('button', { name: 'cancel' }));
		expect(onCancel).toHaveBeenCalledTimes(1);
	});

	it('calls onCancel when Escape is pressed', async () => {
		const onCancel = vi.fn();
		render(Harness, {
			props: { open: true, onCancel, onConfirm: () => {} }
		});

		// Keydown is listened on the backdrop element; dispatch there.
		const dialog = screen.getByRole('alertdialog');
		const backdrop = dialog.parentElement as HTMLElement;
		await fireEvent.keyDown(backdrop, { key: 'Escape' });

		expect(onCancel).toHaveBeenCalledTimes(1);
	});

	it('calls onCancel when the backdrop is clicked', async () => {
		const onCancel = vi.fn();
		render(Harness, {
			props: { open: true, onCancel, onConfirm: () => {} }
		});

		const dialog = screen.getByRole('alertdialog');
		const backdrop = dialog.parentElement as HTMLElement;

		// Dispatch pointerdown on the backdrop element itself (not the dialog).
		await fireEvent.pointerDown(backdrop);
		expect(onCancel).toHaveBeenCalledTimes(1);
	});

	it('does NOT call onCancel when the dialog body is clicked', async () => {
		const onCancel = vi.fn();
		render(Harness, {
			props: { open: true, onCancel, onConfirm: () => {} }
		});

		const dialog = screen.getByRole('alertdialog');
		// Clicking the dialog (not a button inside it) should not bubble to backdrop.
		await fireEvent.pointerDown(dialog);
		expect(onCancel).not.toHaveBeenCalled();
	});

	it('Tab from confirm cycles back to cancel', async () => {
		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm: () => {} }
		});

		await tick();
		await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));

		const cancelBtn = screen.getByRole('button', { name: 'cancel' });
		const confirmBtn = screen.getByRole('button', { name: 'delete thing' });

		// Focus confirm (the last focusable), press Tab → should wrap to cancel.
		confirmBtn.focus();
		const dialog = screen.getByRole('alertdialog');
		const backdrop = dialog.parentElement as HTMLElement;
		await fireEvent.keyDown(backdrop, { key: 'Tab' });

		expect(document.activeElement).toBe(cancelBtn);
	});

	it('Shift+Tab from cancel cycles to confirm', async () => {
		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm: () => {} }
		});

		await tick();
		await new Promise((resolve) => queueMicrotask(() => resolve(undefined)));

		const cancelBtn = screen.getByRole('button', { name: 'cancel' });
		const confirmBtn = screen.getByRole('button', { name: 'delete thing' });

		// Focus cancel (the first focusable), press Shift+Tab → should wrap to confirm.
		cancelBtn.focus();
		const dialog = screen.getByRole('alertdialog');
		const backdrop = dialog.parentElement as HTMLElement;
		await fireEvent.keyDown(backdrop, { key: 'Tab', shiftKey: true });

		expect(document.activeElement).toBe(confirmBtn);
	});

	it('honours prefers-reduced-motion at the JS layer', async () => {
		// With matchMedia returning matches=true, the runtime detection should
		// short-circuit transition durations. We can't observe the actual
		// animation easily in jsdom, so we assert the read happened by spying.
		const matchMediaSpy = vi.fn().mockImplementation((query: string) => ({
			matches: true,
			media: query,
			onchange: null,
			addListener: vi.fn(),
			removeListener: vi.fn(),
			addEventListener: vi.fn(),
			removeEventListener: vi.fn(),
			dispatchEvent: vi.fn()
		}));
		Object.defineProperty(window, 'matchMedia', {
			writable: true,
			configurable: true,
			value: matchMediaSpy
		});

		render(Harness, {
			props: { open: true, onCancel: () => {}, onConfirm: () => {} }
		});

		await tick();

		// The component reads matchMedia once on open. Confirm the query.
		expect(matchMediaSpy).toHaveBeenCalledWith('(prefers-reduced-motion: reduce)');
	});
});
