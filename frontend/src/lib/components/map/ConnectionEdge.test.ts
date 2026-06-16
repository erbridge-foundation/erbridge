import { render, screen, cleanup } from '@testing-library/svelte';
import { afterEach, describe, it, expect } from 'vitest';
import ConnectionEdgeLabel from './ConnectionEdgeLabel.svelte';
import type { Mass } from '$lib/map/types';

// globals: false in vitest.config → testing-library's auto-cleanup afterEach
// isn't registered; do it explicitly so a prior render's ⚠ doesn't leak.
afterEach(cleanup);

// The encoding under test lives in ConnectionEdgeLabel (factored out of
// ConnectionEdge so it renders without SvelteFlow's edge pipeline, which needs
// measured node dimensions jsdom can't provide). ConnectionEdge itself is thin
// plumbing (getBezierPath + EdgeLabelRenderer) — covered by the e2e canvas test.

function renderLabel(props: { wh_type: string; mass: Mass; eol: boolean }) {
	return render(ConnectionEdgeLabel, { props });
}

describe('ConnectionEdge encoding (meaning never colour-only)', () => {
	it('renders the wormhole type as TEXT', () => {
		renderLabel({ wh_type: 'C247', mass: 'fresh', eol: false });
		expect(screen.getByText('C247')).toBeInTheDocument();
	});

	it('renders mass as a TEXT cue, not colour alone (half)', () => {
		renderLabel({ wh_type: 'D845', mass: 'half', eol: false });
		expect(screen.getByText('half')).toBeInTheDocument();
	});

	it('renders mass as a TEXT cue, not colour alone (critical)', () => {
		renderLabel({ wh_type: 'N968', mass: 'critical', eol: false });
		expect(screen.getByText('critical')).toBeInTheDocument();
	});

	it('conveys end-of-life with the ⚠ glyph and text, independent of the pulse', () => {
		const { container } = renderLabel({ wh_type: 'X702', mass: 'half', eol: true });
		// The glyph is present...
		expect(screen.getByText('⚠')).toBeInTheDocument();
		// ...as is a screen-reader text label (so EoL survives loss of the pulse).
		expect(screen.getByText('end of life')).toBeInTheDocument();
		// The pulse is decorative (aria-hidden); its removal under reduced-motion
		// loses no information.
		const pulse = container.querySelector('.pulse');
		expect(pulse).not.toBeNull();
		expect(pulse?.getAttribute('aria-hidden')).toBe('true');
	});

	it('omits the EoL flag for a non-EoL connection', () => {
		renderLabel({ wh_type: 'K162', mass: 'fresh', eol: false });
		expect(screen.queryByText('⚠')).toBeNull();
	});
});
