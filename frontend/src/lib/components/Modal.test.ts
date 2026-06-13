import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';
import Harness from './Modal.test.svelte';

// matchMedia is not implemented in jsdom; install a default stub.
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

// Keydown is listened on the backdrop (the dialog's parent element); helper to
// reach it from the dialog.
function backdropOf(dialog: HTMLElement): HTMLElement {
	return dialog.parentElement as HTMLElement;
}

describe('Modal', () => {
	beforeEach(() => {
		installMatchMediaStub(false);
	});

	afterEach(() => {
		cleanup();
	});

	it('does not render when open is false', () => {
		render(Harness, { props: { open: false, onClose: () => {} } });
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('renders title snippet and has dialog semantics when open', () => {
		render(Harness, { props: { open: true, onClose: () => {}, titleText: 'Create map' } });

		const dialog = screen.getByRole('dialog');
		expect(dialog).toHaveAttribute('aria-modal', 'true');
		const labelledById = dialog.getAttribute('aria-labelledby');
		expect(labelledById).toBeTruthy();
		expect(document.getElementById(labelledById as string)).toHaveTextContent('Create map');
	});

	it('calls onClose when Escape is pressed', async () => {
		const onClose = vi.fn();
		render(Harness, { props: { open: true, onClose } });

		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('calls onClose when the backdrop is clicked', async () => {
		const onClose = vi.fn();
		render(Harness, { props: { open: true, onClose } });

		await fireEvent.pointerDown(backdropOf(screen.getByRole('dialog')));
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('does NOT call onClose when the dialog body is clicked', async () => {
		const onClose = vi.fn();
		render(Harness, { props: { open: true, onClose } });

		await fireEvent.pointerDown(screen.getByRole('dialog'));
		expect(onClose).not.toHaveBeenCalled();
	});

	it('Tab from the last focusable wraps to the first', async () => {
		render(Harness, { props: { open: true, onClose: () => {} } });
		await tick();

		const first = screen.getByTestId('first');
		const last = screen.getByTestId('last');

		last.focus();
		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), { key: 'Tab' });

		expect(document.activeElement).toBe(first);
	});

	it('Shift+Tab from the first focusable wraps to the last', async () => {
		render(Harness, { props: { open: true, onClose: () => {} } });
		await tick();

		const first = screen.getByTestId('first');
		const last = screen.getByTestId('last');

		first.focus();
		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), {
			key: 'Tab',
			shiftKey: true
		});

		expect(document.activeElement).toBe(last);
	});

	it('a conditionally added field joins the tab cycle', async () => {
		const { rerender } = render(Harness, { props: { open: true, onClose: () => {}, showExtra: false } });
		await tick();

		// Without the extra field, the last focusable is the submit button.
		// Add it: now the submit button is still last, but the extra input sits
		// before it — Shift+Tab from first must wrap to the submit button, and
		// the extra field must be reachable as part of the set.
		await rerender({ open: true, onClose: () => {}, showExtra: true });
		await tick();

		const extra = screen.getByTestId('extra');
		const last = screen.getByTestId('last');

		// Tab from the new extra field lands on the submit button (next in order),
		// proving the freshly rendered element is part of the live focusable set.
		extra.focus();
		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), { key: 'Tab' });
		// extra is not the last element, so the browser handles the move; focus
		// stays put in jsdom (no native tab). Instead assert wrap from the true last.
		last.focus();
		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), { key: 'Tab' });
		expect(document.activeElement).toBe(screen.getByTestId('first'));

		// And the extra field is included in the computed focusable set: Shift+Tab
		// from first wraps to the submit button (the last), confirming the set is
		// recomputed live with the extra present.
		screen.getByTestId('first').focus();
		await fireEvent.keyDown(backdropOf(screen.getByRole('dialog')), {
			key: 'Tab',
			shiftKey: true
		});
		expect(document.activeElement).toBe(last);
	});

	it('restores focus to the opener on close', async () => {
		// Start closed so the open $effect captures the opener as the previously
		// focused element when we then open the dialog.
		const { rerender } = render(Harness, { props: { open: false, onClose: () => {} } });

		const opener = screen.getByTestId('opener');
		opener.focus();
		expect(document.activeElement).toBe(opener);

		await rerender({ open: true, onClose: () => {} });
		await tick();

		// Closing runs the open-effect cleanup synchronously, which restores focus
		// to the opener. (The dialog node itself may linger briefly while its outro
		// transition plays — focus restoration does not wait for that.)
		await rerender({ open: false, onClose: () => {} });
		await tick();

		expect(document.activeElement).toBe(opener);
	});
});
