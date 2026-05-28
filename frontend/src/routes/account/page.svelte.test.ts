// Coverage of the inline reveal-panel gate. ConfirmDialog has its own tests
// (src/lib/components/ConfirmDialog.test.ts) and is not re-tested here.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';

vi.mock('$app/forms', () => ({
	enhance: () => ({ destroy: () => {} })
}));

vi.mock('$app/navigation', () => ({
	invalidateAll: vi.fn(async () => {})
}));

const AccountPage = (await import('./+page.svelte')).default;
import type { ComponentProps } from 'svelte';

const PLAINTEXT = 'erb_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

function baseProps(opts: { form?: unknown; keys?: unknown[] } = {}): ComponentProps<typeof AccountPage> {
	return {
		data: { keys: opts.keys ?? [] },
		form: opts.form ?? null
	} as unknown as ComponentProps<typeof AccountPage>;
}

const createdForm = {
	createdKey: {
		id: 'k1',
		key: PLAINTEXT,
		name: 'ci',
		expires_at: null,
		created_at: '2026-05-28T10:00:00Z'
	}
};

beforeEach(() => {
	// Reset clipboard mock between tests.
	Object.defineProperty(navigator, 'clipboard', {
		value: { writeText: vi.fn(async () => {}) },
		configurable: true
	});
});

afterEach(() => cleanup());

describe('/account reveal panel', () => {
	it('renders the plaintext key, Copy, the unchecked ack box, and a disabled Done', async () => {
		render(AccountPage, { props: baseProps({ form: createdForm }) });
		await tick();

		expect(screen.getByTestId('reveal-panel')).toBeInTheDocument();
		expect(screen.getByTestId('reveal-key').textContent).toBe(PLAINTEXT);

		const ack = screen.getByTestId('reveal-ack') as HTMLInputElement;
		expect(ack.checked).toBe(false);

		const done = screen.getByTestId('reveal-done') as HTMLButtonElement;
		expect(done.disabled).toBe(true);
	});

	it('enables Done once the ack checkbox is ticked', async () => {
		render(AccountPage, { props: baseProps({ form: createdForm }) });
		await tick();

		const ack = screen.getByTestId('reveal-ack') as HTMLInputElement;
		const done = screen.getByTestId('reveal-done') as HTMLButtonElement;

		await fireEvent.click(ack);
		await tick();

		expect(ack.checked).toBe(true);
		expect(done.disabled).toBe(false);
	});

	it('unmounts the panel and clears the plaintext from the DOM when Done is activated', async () => {
		const { container } = render(AccountPage, { props: baseProps({ form: createdForm }) });
		await tick();

		await fireEvent.click(screen.getByTestId('reveal-ack'));
		await tick();
		await fireEvent.click(screen.getByTestId('reveal-done'));
		await tick();

		expect(screen.queryByTestId('reveal-panel')).toBeNull();
		expect(screen.queryByTestId('reveal-key')).toBeNull();
		// Defence-in-depth: the plaintext must not be anywhere in the DOM.
		expect(container.innerHTML).not.toContain(PLAINTEXT);
	});

	it('shows the manual-select hint when navigator.clipboard.writeText rejects, and the gate still functions', async () => {
		Object.defineProperty(navigator, 'clipboard', {
			value: { writeText: vi.fn(async () => Promise.reject(new Error('denied'))) },
			configurable: true
		});

		render(AccountPage, { props: baseProps({ form: createdForm }) });
		await tick();

		const copyBtn = screen.getByTestId('reveal-copy');
		await fireEvent.click(copyBtn);
		await tick();
		// Wait one more microtask for the rejection to propagate through the
		// async event handler.
		await Promise.resolve();
		await tick();

		expect(
			screen.getByText("Couldn't copy automatically — select the key and copy it manually.")
		).toBeInTheDocument();

		// Gate still works after a copy failure.
		const ack = screen.getByTestId('reveal-ack') as HTMLInputElement;
		const done = screen.getByTestId('reveal-done') as HTMLButtonElement;
		await fireEvent.click(ack);
		await tick();
		expect(done.disabled).toBe(false);
	});

	it('does not render the reveal panel when there is no createdKey in form', async () => {
		render(AccountPage, { props: baseProps({ form: null }) });
		await tick();
		expect(screen.queryByTestId('reveal-panel')).toBeNull();
	});
});
