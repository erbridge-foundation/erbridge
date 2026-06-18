import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/svelte';
import Harness from './DialogActions.test.svelte';

afterEach(cleanup);

describe('DialogActions', () => {
	it('renders the consumer-supplied buttons in the footer row', () => {
		const { container } = render(Harness);
		// The shared footer wrapper is present...
		expect(container.querySelector('.dialog-actions')).not.toBeNull();
		// ...and the consumer's own buttons (with their own types) render inside it.
		const cancel = screen.getByRole('button', { name: 'Cancel' });
		const save = screen.getByRole('button', { name: 'Save' });
		expect(cancel).toHaveAttribute('type', 'button');
		expect(save).toHaveAttribute('type', 'submit');
		expect(container.querySelector('.dialog-actions')?.contains(cancel)).toBe(true);
	});
});
