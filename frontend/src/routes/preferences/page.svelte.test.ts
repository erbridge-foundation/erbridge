import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';

// Mock the store so we can assert preview vs commit without real localStorage/fetch.
const mockState = { current: {
	text_size: 'auto',
	reduce_motion: 'auto',
	high_contrast: 'auto',
	large_targets: 'off',
	dyslexia_font: 'off'
} };
const preview = vi.fn();
const commit = vi.fn();
const revertToPersisted = vi.fn();

vi.mock('$lib/preferences/store.svelte', () => ({
	preferences: {
		get current() {
			return mockState.current;
		},
		preview,
		commit,
		revertToPersisted
	}
}));

const PreferencesPage = (await import('./+page.svelte')).default;

beforeEach(() => {
	preview.mockClear();
	commit.mockClear();
	revertToPersisted.mockClear();
});

afterEach(() => cleanup());

/** Find the option button with `name` inside the fieldset whose legend is `legend`. */
function optionInGroup(legend: string, name: string): HTMLButtonElement {
	const fieldset = screen.getByText(legend).closest('fieldset')!;
	const btn = Array.from(fieldset.querySelectorAll('button[role="radio"]')).find(
		(b) => b.textContent?.trim() === name
	);
	if (!btn) throw new Error(`option "${name}" not found in group "${legend}"`);
	return btn as HTMLButtonElement;
}

describe('/preferences page', () => {
	it('commits reduce_motion immediately (no revert bar)', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Reduce motion', 'On'));
		await tick();

		expect(commit).toHaveBeenCalledWith({ reduce_motion: 'on' });
		expect(preview).not.toHaveBeenCalled();
		expect(screen.queryByText(/Keeping these settings/)).toBeNull();
	});

	it('previews a layout-altering change and shows the revert bar (no commit yet)', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();

		expect(preview).toHaveBeenCalledWith({ text_size: 'large' });
		expect(commit).not.toHaveBeenCalled();
		expect(screen.getByText(/Keeping these settings/)).toBeInTheDocument();
	});

	it('Keep commits the previewed change', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();
		await fireEvent.click(screen.getByText('Keep'));
		await tick();

		expect(commit).toHaveBeenCalledWith({ text_size: 'large' });
	});

	it('Revert now discards the preview without committing', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();
		await fireEvent.click(screen.getByText('Revert now'));
		await tick();

		expect(revertToPersisted).toHaveBeenCalledTimes(1);
		expect(commit).not.toHaveBeenCalled();
	});
});
