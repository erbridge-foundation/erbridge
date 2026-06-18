import { render, screen, cleanup } from '@testing-library/svelte';
import { afterEach, describe, it, expect } from 'vitest';
import ConnectionEdgeLabel from './ConnectionEdgeLabel.svelte';
import { resolveEdgeEncoding } from '$lib/map/edge-encoding';
import type { Mass, TtlState } from '$lib/map/types';

// globals: false in vitest.config → testing-library's auto-cleanup afterEach
// isn't registered; do it explicitly so a prior render's label doesn't leak.
afterEach(cleanup);

// The colour-INDEPENDENT encoding under test lives in ConnectionEdgeLabel
// (factored out of ConnectionEdge so it renders without SvelteFlow's edge
// pipeline, which needs measured node dimensions jsdom can't provide).
// ConnectionEdge itself is thin plumbing (BaseEdge + casing + EdgeLabel + the
// midpoint direction arrow) — covered by the e2e canvas test. The mass+ttl→channel
// resolver is unit-tested separately in edge-encoding.test.ts.

function renderLabel(props: {
	wh_type: string;
	mass: Mass;
	ttlBucket: TtlState;
	alertLevel?: 'none' | 'warning' | 'danger';
	showMass?: boolean;
	showWhType?: boolean;
}) {
	return render(ConnectionEdgeLabel, {
		props: {
			alertLevel: 'none' as const,
			...props
		}
	});
}

describe('ConnectionEdgeLabel encoding (meaning never colour-only)', () => {
	it('renders the wormhole type as TEXT', () => {
		renderLabel({ wh_type: 'C247', mass: 'fresh', ttlBucket: 'stable' });
		expect(screen.getByText('C247')).toBeInTheDocument();
	});

	it('renders mass as a TEXT cue, not colour alone (half)', () => {
		renderLabel({ wh_type: 'D845', mass: 'half', ttlBucket: 'stable' });
		expect(screen.getByText('half')).toBeInTheDocument();
	});

	it('renders mass as a TEXT cue, not colour alone (critical)', () => {
		renderLabel({ wh_type: 'N968', mass: 'critical', ttlBucket: 'stable' });
		expect(screen.getByText('critical')).toBeInTheDocument();
	});

	it('emits NO TTL text for a stable connection (calm baseline)', () => {
		const { container } = renderLabel({
			wh_type: 'K162',
			mass: 'fresh',
			ttlBucket: 'stable'
		});
		expect(container.querySelector('.sr-only')).toBeNull();
	});

	it('conveys low-TTL as sr-only text (survives loss of colour AND motion)', () => {
		const { container } = renderLabel({
			wh_type: 'X702',
			mass: 'half',
			ttlBucket: 'lt1h',
			alertLevel: 'warning'
		});
		// The mid-edge glyph is gone; the precise four-state text carries the state for
		// screen-reader / forced-colors / reduced-motion users.
		expect(container.querySelector('.sr-only')).not.toBeNull();
		expect(screen.getByText('less than 1 hour')).toBeInTheDocument();
	});

	it('keeps the precise four-state distinction in the sr-only text (imminent)', () => {
		renderLabel({
			wh_type: 'B274',
			mass: 'fresh',
			ttlBucket: 'imminent',
			alertLevel: 'danger'
		});
		expect(screen.getByText('closure imminent')).toBeInTheDocument();
	});

	it('emits the TTL text even when the mass + wh-type cues are toggled off', () => {
		// showLabel upstream keeps the label mounted on an alert; the sr-only text must
		// still be present so the state survives a label stripped of its visible cues.
		renderLabel({
			wh_type: 'B274',
			mass: 'fresh',
			ttlBucket: 'imminent',
			alertLevel: 'danger',
			showMass: false,
			showWhType: false
		});
		expect(screen.getByText('closure imminent')).toBeInTheDocument();
	});

	it('drops the pill chrome when no visible cue renders (no residual empty pill)', () => {
		// Both cues off + only the sr-only TTL text → the label must NOT paint its
		// border/background (that was the residual empty-pill bug on alert edges).
		const { container } = renderLabel({
			wh_type: 'B274',
			mass: 'fresh',
			ttlBucket: 'imminent',
			alertLevel: 'danger',
			showMass: false,
			showWhType: false
		});
		expect(container.querySelector('.edge-label.chrome')).toBeNull();
	});

	it('keeps the chrome when a visible cue renders', () => {
		const { container } = renderLabel({
			wh_type: 'C247',
			mass: 'half',
			ttlBucket: 'lt4h',
			alertLevel: 'warning'
		});
		expect(container.querySelector('.edge-label.chrome')).not.toBeNull();
	});

	it('drops the chrome when wh-type is empty and mass is off (no text to show)', () => {
		// The named-type derivation yields '' for an all-K162 / unscanned hole; with
		// mass off there is nothing visible, so no pill.
		const { container } = renderLabel({
			wh_type: '',
			mass: 'fresh',
			ttlBucket: 'lt4h',
			alertLevel: 'warning',
			showMass: false
		});
		expect(container.querySelector('.edge-label.chrome')).toBeNull();
	});
});

describe('resolveEdgeEncoding (the one config object)', () => {
	it('mass owns thickness: fresh > half > critical, critical floored at 2', () => {
		expect(resolveEdgeEncoding('fresh', 2000).mass.width).toBe(5);
		expect(resolveEdgeEncoding('half', 2000).mass.width).toBe(3);
		expect(resolveEdgeEncoding('critical', 2000).mass.width).toBe(2);
	});

	it('buckets remaining-minutes into the four TTL states (anything above 4h is stable)', () => {
		expect(resolveEdgeEncoding('fresh', 300).ttlBucket).toBe('stable');
		expect(resolveEdgeEncoding('fresh', 200).ttlBucket).toBe('lt4h');
		expect(resolveEdgeEncoding('fresh', 45).ttlBucket).toBe('lt1h');
		expect(resolveEdgeEncoding('fresh', 5).ttlBucket).toBe('imminent');
	});

	it('collapses the four TTL states onto three VISUAL tiers', () => {
		// above 4h → calm; < 4h → warning; < 1h AND imminent → the SAME critical.
		expect(resolveEdgeEncoding('fresh', 300).ttlVisual).toBe('calm');
		expect(resolveEdgeEncoding('fresh', 200).ttlVisual).toBe('warning');
		expect(resolveEdgeEncoding('fresh', 45).ttlVisual).toBe('critical');
		expect(resolveEdgeEncoding('fresh', 5).ttlVisual).toBe('critical');
	});

	it('stable has no dash (solid, calm)', () => {
		const enc = resolveEdgeEncoding('fresh', 2000);
		expect(enc.ttl.dashArray).toBe('');
		expect(enc.alert.level).toBe('none');
	});

	it('fires NO alert for a fresh + stable edge', () => {
		expect(resolveEdgeEncoding('fresh', 2000).alert.level).toBe('none');
	});

	it('< 4h is the gentle WARNING tier (amber dash, gentle breath)', () => {
		const enc = resolveEdgeEncoding('fresh', 180);
		expect(enc.alert.level).toBe('warning');
		expect(enc.ttl.dashArray).toBe('14 8');
		expect(enc.alert.breatheClass).toBe('halo-amber');
	});

	it('< 1h is the CRITICAL tier — the loud red danger signal (not warning)', () => {
		const enc = resolveEdgeEncoding('half', 45);
		expect(enc.ttlBucket).toBe('lt1h');
		expect(enc.alert.level).toBe('danger');
		expect(enc.alert.breatheClass).toBe('halo-red');
		expect(enc.ttl.dashArray).toBe('9 9 2 9');
	});

	it('imminent renders IDENTICALLY to < 1h (same critical visual)', () => {
		const lt1h = resolveEdgeEncoding('half', 45);
		const imminent = resolveEdgeEncoding('half', 5);
		expect(imminent.ttl).toEqual(lt1h.ttl);
		expect(imminent.alert).toEqual(lt1h.alert);
		// ...but the precise four-state bucket still differs for text/sorting.
		expect(lt1h.ttlBucket).toBe('lt1h');
		expect(imminent.ttlBucket).toBe('imminent');
	});

	it('alert is TTL-driven: a fresh-mass < 1h edge is danger and breathes', () => {
		const enc = resolveEdgeEncoding('fresh', 45);
		expect(enc.alert.level).toBe('danger');
		expect(enc.alert.breatheClass).toBe('halo-red');
	});

	it('critical MASS with healthy time gets NO casing — the glow is pure TTL', () => {
		// The thin red line already conveys critical mass; the halo is reserved for
		// the time axis, so a time-stable crit-mass hole does not glow.
		const enc = resolveEdgeEncoding('critical', 2000);
		expect(enc.alert.level).toBe('none');
		expect(enc.alert.breatheClass).toBe('');
	});

	it('critical-tier dash-dot is retained even on the thinnest (critical-mass) line', () => {
		const enc = resolveEdgeEncoding('critical', 5);
		expect(enc.mass.width).toBe(2);
		expect(enc.ttl.dashArray).toBe('9 9 2 9');
	});
});
