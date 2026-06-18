import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import MapPreferences from './MapPreferences.svelte';

// globals: false in vitest.config → register cleanup explicitly.
afterEach(cleanup);

function renderPrefs(overrides: Record<string, unknown> = {}) {
	return render(MapPreferences, {
		props: {
			open: true,
			thickness: 2,
			thicknessMin: 1,
			thicknessMax: 8,
			showMass: true,
			showWhType: true,
			showSignatures: true,
			showDirection: true,
			layoutDir: 'LR' as const,
			autoLayout: false,
			onSelectLayout: () => {},
			...overrides
		}
	});
}

describe('MapPreferences', () => {
	it('renders the display-preference controls when open', () => {
		renderPrefs();
		expect(screen.getByRole('dialog', { name: 'Map preferences' })).toBeInTheDocument();
		// The four label toggles + the edge-thickness slider + the layout style picker.
		expect(screen.getByLabelText('Mass labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Type labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Signature labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Show direction')).toBeInTheDocument();
		expect(screen.getByRole('slider', { name: 'Edge thickness' })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Left → right' })).toBeInTheDocument();
		expect(screen.getByLabelText('Auto-layout on changes')).toBeInTheDocument();
	});

	it('does NOT render the throwaway colour-blind toggle (that stays in the sidebar)', () => {
		renderPrefs();
		expect(screen.queryByLabelText('Colour-blind palette')).toBeNull();
	});

	it('marks the active layout style as pressed', () => {
		renderPrefs({ layoutDir: 'TB' });
		expect(screen.getByRole('button', { name: 'Top → bottom' })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		expect(screen.getByRole('button', { name: 'Left → right' })).toHaveAttribute(
			'aria-pressed',
			'false'
		);
	});

	it('calls onSelectLayout when a style is picked', async () => {
		const onSelectLayout = vi.fn();
		renderPrefs({ onSelectLayout });
		await fireEvent.click(screen.getByRole('button', { name: 'Bottom → top' }));
		expect(onSelectLayout).toHaveBeenCalledWith('BT');
	});

	it('renders nothing when closed', () => {
		renderPrefs({ open: false });
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('has Cancel + OK footer buttons', () => {
		renderPrefs();
		expect(screen.getByRole('button', { name: 'cancel' })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'OK' })).toBeInTheDocument();
	});

	it('OK does not revert the layout style (keeps live edits)', async () => {
		// On open the snapshot equals the current style, so neither OK nor Cancel should
		// fire onSelectLayout when nothing changed — OK in particular must never revert.
		const onSelectLayout = vi.fn();
		renderPrefs({ layoutDir: 'LR', onSelectLayout });
		await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
		expect(onSelectLayout).not.toHaveBeenCalled();
	});

	it('Cancel does not fire onSelectLayout when the style is unchanged since open', async () => {
		// Cancel reverts the style ONLY if it changed; unchanged → no spurious reflow.
		// (The behavioural revert of bindable prefs + a changed style is covered against
		// the live canvas in the e2e suite, where two-way binding actually applies.)
		const onSelectLayout = vi.fn();
		renderPrefs({ layoutDir: 'TB', onSelectLayout });
		await fireEvent.click(screen.getByRole('button', { name: 'cancel' }));
		expect(onSelectLayout).not.toHaveBeenCalled();
	});
});
