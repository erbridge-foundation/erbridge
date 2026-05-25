import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';

const DEFAULTS = {
	text_size: 'auto',
	reduce_motion: 'auto',
	high_contrast: 'auto',
	large_targets: 'off',
	dyslexia_font: 'off'
} as const;

// Mutable persisted baseline the mocked store reports; tests set it before render.
const store = { persisted: { ...DEFAULTS } as Record<string, string> };
const preview = vi.fn();
const commit = vi.fn(async () => {});
const revertToPersisted = vi.fn();
const resetToDefaults = vi.fn(async () => {});

vi.mock('$lib/preferences/store.svelte', () => ({
	preferences: {
		get persisted() {
			return store.persisted;
		},
		get current() {
			return store.persisted;
		},
		preview,
		commit,
		revertToPersisted,
		resetToDefaults
	}
}));

// Capture the beforeNavigate callback so we can invoke it in a test.
let beforeNavigateCb: (() => void) | null = null;
vi.mock('$app/navigation', () => ({
	beforeNavigate: (cb: () => void) => {
		beforeNavigateCb = cb;
	}
}));

const PreferencesPage = (await import('./+page.svelte')).default;

beforeEach(() => {
	store.persisted = { ...DEFAULTS };
	preview.mockClear();
	commit.mockClear();
	revertToPersisted.mockClear();
	resetToDefaults.mockClear();
	beforeNavigateCb = null;
});

afterEach(() => cleanup());

function optionInGroup(legend: string, name: string): HTMLButtonElement {
	const fieldset = screen.getByText(legend).closest('fieldset')!;
	const btn = Array.from(fieldset.querySelectorAll('button[role="radio"]')).find(
		(b) => b.textContent?.trim() === name
	);
	if (!btn) throw new Error(`option "${name}" not found in group "${legend}"`);
	return btn as HTMLButtonElement;
}

describe('/preferences page — staging model', () => {
	it('selecting a control previews live but does not persist', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();

		expect(preview).toHaveBeenCalledWith(expect.objectContaining({ text_size: 'large' }));
		expect(commit).not.toHaveBeenCalled();
	});

	it('shows Apply/Discard only when dirty', async () => {
		render(PreferencesPage);
		// Clean initially — no Apply/Discard, but Reset is always present.
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Discard' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Reset to defaults' })).toBeInTheDocument();

		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();

		expect(screen.getByRole('button', { name: 'Apply' })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Discard' })).toBeInTheDocument();
	});

	it('Apply commits the staged batch (including reduce_motion)', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await fireEvent.click(optionInGroup('Reduce motion', 'On'));
		await tick();
		await fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
		await tick();

		expect(commit).toHaveBeenCalledTimes(1);
		expect(commit).toHaveBeenCalledWith(
			expect.objectContaining({ text_size: 'large', reduce_motion: 'on' })
		);
	});

	it('reduce_motion stages (does not commit instantly)', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Reduce motion', 'On'));
		await tick();

		expect(commit).not.toHaveBeenCalled();
		expect(preview).toHaveBeenCalledWith(expect.objectContaining({ reduce_motion: 'on' }));
		expect(screen.getByRole('button', { name: 'Apply' })).toBeInTheDocument();
	});

	it('Discard reverts the previews and clears the dirty state', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();
		await fireEvent.click(screen.getByRole('button', { name: 'Discard' }));
		await tick();

		expect(revertToPersisted).toHaveBeenCalledTimes(1);
		expect(commit).not.toHaveBeenCalled();
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
	});

	it('returning a control to its persisted value clears the dirty state', async () => {
		store.persisted = { ...DEFAULTS, text_size: 'large' };
		render(PreferencesPage);

		// Change away from persisted → dirty.
		await fireEvent.click(optionInGroup('Text size', 'Small'));
		await tick();
		expect(screen.getByRole('button', { name: 'Apply' })).toBeInTheDocument();

		// Back to the persisted value → clean again, no special-case.
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Discard' })).toBeNull();
	});

	it('Reset to defaults is always available and calls resetToDefaults', async () => {
		render(PreferencesPage);
		await fireEvent.click(screen.getByRole('button', { name: 'Reset to defaults' }));
		await tick();
		expect(resetToDefaults).toHaveBeenCalledTimes(1);
	});

	it('navigating away while dirty discards the previews', async () => {
		render(PreferencesPage);
		await fireEvent.click(optionInGroup('Text size', 'Large'));
		await tick();

		// Simulate the SvelteKit beforeNavigate firing.
		expect(beforeNavigateCb).not.toBeNull();
		beforeNavigateCb!();

		expect(revertToPersisted).toHaveBeenCalled();
	});

	it('does not discard on navigate when clean', async () => {
		render(PreferencesPage);
		beforeNavigateCb!();
		expect(revertToPersisted).not.toHaveBeenCalled();
	});
});
