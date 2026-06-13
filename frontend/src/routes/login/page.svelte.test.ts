import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import {
	DEFAULT_PREFERENCES,
	MAX_PREFERENCES,
	type PreferencesPatch
} from '$lib/preferences/schema';

// Mutable current set the mocked store reports; tests set it before render.
const store = { current: { ...DEFAULT_PREFERENCES } as Record<string, string> };
const commit = vi.fn(async (_patch: PreferencesPatch) => {});

vi.mock('$lib/preferences/store.svelte', () => ({
	preferences: {
		get current() {
			return store.current;
		},
		commit
	}
}));

const LoginPage = (await import('./+page.svelte')).default;

beforeEach(() => {
	store.current = { ...DEFAULT_PREFERENCES };
	commit.mockClear();
});

afterEach(() => cleanup());

describe('login accessibility toggle', () => {
	it('is off when the preset is not active', () => {
		render(LoginPage);
		const toggle = screen.getByRole('checkbox', { name: /maximize accessibility/i });
		expect((toggle as HTMLInputElement).checked).toBe(false);
	});

	it('commits all five MAX_PREFERENCES keys when activated', async () => {
		render(LoginPage);
		const toggle = screen.getByRole('checkbox', { name: /maximize accessibility/i });
		await fireEvent.change(toggle, { target: { checked: true } });

		expect(commit).toHaveBeenCalledTimes(1);
		const patch = commit.mock.calls[0][0];
		expect(patch).toEqual({ ...MAX_PREFERENCES });
		// locale is never part of the preset
		expect(patch).not.toHaveProperty('locale');
	});

	it('derives the on-state from the store when the preset is active', () => {
		store.current = { ...DEFAULT_PREFERENCES, ...MAX_PREFERENCES };
		render(LoginPage);
		const toggle = screen.getByRole('checkbox', { name: /maximize accessibility/i });
		expect((toggle as HTMLInputElement).checked).toBe(true);
		// disclosure text shows only when active
		expect(screen.getByText(/applied to this screen/i)).toBeInTheDocument();
	});

	it('reverts the five keys to their defaults when deactivated', async () => {
		store.current = { ...DEFAULT_PREFERENCES, ...MAX_PREFERENCES };
		render(LoginPage);
		const toggle = screen.getByRole('checkbox', { name: /maximize accessibility/i });
		await fireEvent.change(toggle, { target: { checked: false } });

		expect(commit).toHaveBeenCalledTimes(1);
		expect(commit.mock.calls[0][0]).toEqual({
			text_size: DEFAULT_PREFERENCES.text_size,
			high_contrast: DEFAULT_PREFERENCES.high_contrast,
			reduce_motion: DEFAULT_PREFERENCES.reduce_motion,
			large_targets: DEFAULT_PREFERENCES.large_targets,
			dyslexia_font: DEFAULT_PREFERENCES.dyslexia_font
		});
	});

	it('hides the disclosure text when the preset is inactive', () => {
		render(LoginPage);
		expect(screen.queryByText(/applied to this screen/i)).not.toBeInTheDocument();
	});
});

describe('login language picker', () => {
	it('commits only the chosen locale and leaves accessibility keys untouched', async () => {
		render(LoginPage);
		const select = screen.getByLabelText(/language/i);
		await fireEvent.change(select, { target: { value: 'de' } });

		expect(commit).toHaveBeenCalledTimes(1);
		expect(commit.mock.calls[0][0]).toEqual({ locale: 'de' });
	});

	it('reflects the active locale as selected', () => {
		store.current = { ...DEFAULT_PREFERENCES, locale: 'fr' };
		render(LoginPage);
		const select = screen.getByLabelText(/language/i) as HTMLSelectElement;
		expect(select.value).toBe('fr');
	});

	it('does not re-commit when selecting the already-active locale', async () => {
		render(LoginPage); // default locale is 'en'
		const select = screen.getByLabelText(/language/i);
		await fireEvent.change(select, { target: { value: 'en' } });
		expect(commit).not.toHaveBeenCalled();
	});
});
