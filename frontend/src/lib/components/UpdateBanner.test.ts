import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';

// `updated` from `$app/state` is a SvelteKit-provided reactive proxy with a
// `.current` boolean. We control it from each test via this mutable stub.
const updatedStub = { current: false };

vi.mock('$app/state', () => ({
	get updated() {
		return updatedStub;
	}
}));

// Import after the mock is registered.
import UpdateBanner from './UpdateBanner.svelte';

describe('UpdateBanner', () => {
	beforeEach(() => {
		updatedStub.current = false;
	});

	afterEach(() => {
		cleanup();
	});

	it('renders nothing when updated.current is false', () => {
		render(UpdateBanner);

		expect(screen.queryByRole('status')).toBeNull();
		expect(screen.queryByRole('button', { name: 'reload' })).toBeNull();
	});

	it('renders the message and reload control when updated.current is true', () => {
		updatedStub.current = true;
		render(UpdateBanner);

		expect(screen.getByRole('status')).toBeInTheDocument();
		expect(screen.getByText('A new version is available.')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'reload' })).toBeInTheDocument();
	});

	it('invokes the reload handler when the reload control is activated', async () => {
		updatedStub.current = true;
		const onReload = vi.fn();
		render(UpdateBanner, { props: { onReload } });

		await fireEvent.click(screen.getByRole('button', { name: 'reload' }));
		expect(onReload).toHaveBeenCalledTimes(1);
	});
});
