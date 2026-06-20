import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/svelte';
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
			nodeSpacing: 170,
			spacingMin: 100,
			spacingMax: 250,
			layoutAlgo: 'dagre',
			showMass: false,
			showWhType: false,
			showSignatures: true,
			showDirection: true,
			animateDirection: false,
			autoLayout: false,
			...overrides
		}
	});
}

describe('MapPreferences', () => {
	it('renders the display-preference controls when open', () => {
		renderPrefs();
		expect(screen.getByRole('dialog', { name: 'Map preferences' })).toBeInTheDocument();
		// The four label toggles + the edge-thickness slider + the auto-layout toggle.
		// (The layout STYLE picker + apply-now moved out to the tab-bar split-button.)
		expect(screen.getByLabelText('Mass labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Type labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Signature labels')).toBeInTheDocument();
		expect(screen.getByLabelText('Show direction')).toBeInTheDocument();
		expect(screen.getByRole('slider', { name: 'Edge thickness' })).toBeInTheDocument();
		expect(screen.getByRole('slider', { name: 'Node spacing' })).toBeInTheDocument();
		// Auto-layout now lives with the other checkboxes, not in a layout sub-control.
		expect(screen.getByLabelText('Auto-layout on changes')).toBeInTheDocument();
		expect(screen.getByLabelText('Animate direction')).toBeInTheDocument();
	});

	it('renders the layout-engine segmented control with the current engine checked', () => {
		renderPrefs({ layoutAlgo: 'dagre' });
		const dagre = screen.getByRole('radio', { name: 'Dagre' });
		const tidy = screen.getByRole('radio', { name: 'Tidy tree' });
		expect(dagre).toHaveAttribute('aria-checked', 'true');
		expect(tidy).toHaveAttribute('aria-checked', 'false');
	});

	it('disables the animate-direction toggle when direction is hidden', () => {
		renderPrefs({ showDirection: false });
		expect(screen.getByLabelText('Animate direction')).toBeDisabled();
	});

	it('no longer hosts the layout style picker (moved to the tab-bar split-button)', () => {
		renderPrefs();
		expect(screen.queryByRole('button', { name: 'Left → right' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Top → bottom' })).toBeNull();
	});

	it('does NOT render the throwaway colour-blind toggle (that stays in the sidebar)', () => {
		renderPrefs();
		expect(screen.queryByLabelText('Colour-blind palette')).toBeNull();
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
});
