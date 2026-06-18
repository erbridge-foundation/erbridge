/**
 * The ONE config object that resolves a connection's two independent variables
 * (mass + TTL) plus the derived alert into every visual channel of an edge —
 * see the wormhole edge-encoding spec. Pure + framework-free so it is unit-
 * testable without SvelteFlow's edge pipeline (which needs measured node sizes
 * jsdom can't supply); ConnectionEdge.svelte is then thin plumbing over this.
 *
 * Channel ownership (the spec's key principle — separate READING from ALERTING):
 *   MASS  → line thickness + colour.        (a thin red line is its own alarm)
 *   TTL   → the breathing casing (halo) ALONE.  (calm = none / warning / critical)
 *   LABEL → border + sr-text echo the TTL alert level.
 *
 * TTL is carried PURELY by the pulsing background casing now — the dashed-line
 * texture was dropped (it competed with mass colour and read as visual noise). The
 * line is always solid; the halo pulses for warning/critical and is calm (absent)
 * above 4 h. Under prefers-reduced-motion the pulse FREEZES at its MAX width (the
 * loudest, largest state — not a midpoint), and warning vs critical freeze at
 * DISTINCT sizes so the tiers stay tellable apart without colour or motion (plus the
 * sr-only four-state text + the optional mass/time labels). The midpoint is home to
 * the rotated direction arrow. A fresh + stable edge fires NOTHING and stays calm.
 */

import type { Mass, TtlState, TtlVisual } from './types';
import { ttlState, ttlVisual } from './types';

/** Standard vs colour-blind. The ONLY thing it swaps is the three mass hues —
 *  thickness, dash, glyph, motion, and the alert layer are identical across
 *  palettes (so swapping is a one-line change). */
export type Palette = 'standard' | 'colourblind';

/** Resolved mass channels: how wide + what colour the main line is. */
export interface MassEncoding {
	/** Stroke width in px. Critical is floored at 2 so the imminent dash-dot
	 *  doesn't collapse to solid on the thinnest line (spec §2). */
	width: number;
	/** A CSS custom-property reference; the actual hex lives in the palette tokens
	 *  (app.css) so a palette swap is a token swap, not a recompute here. */
	colourVar: string;
}

/** The derived alert layer — the casing (under-stroke halo) + label badge that
 *  own "attention", and the SOLE TTL channel now (the dashed line was dropped).
 *  Fires on the TTL visual ALONE (mass plays no part — see resolveAlert);
 *  `level === 'none'` is calm. */
export interface AlertEncoding {
	level: 'none' | 'warning' | 'danger';
	/** Casing colour token (only meaningful when level !== 'none'). */
	casingColourVar: string;
	/** Casing MAX width / opacity = the breath PEAK, and the resting state the
	 *  component renders inline. The breathing animation swells UP TO this from a
	 *  smaller trough, so a reduced-motion freeze lands on the max (loudest) state.
	 *  warning vs danger have DISTINCT max widths so the frozen tiers are tellable
	 *  apart by SIZE, not colour alone. */
	casingWidth: number;
	casingOpacity: number;
	/** Which breathing keyframe class to apply, or `''` for no motion. Under
	 *  prefers-reduced-motion the global app.css rule freezes the animation, leaving
	 *  the casing at its inline MAX (peak) state. */
	breatheClass: '' | 'halo-amber' | 'halo-red';
}

/** Everything an edge needs to render, resolved from the two raw variables. */
export interface EdgeEncoding {
	mass: MassEncoding;
	alert: AlertEncoding;
	/** The four-state bucketed TTL, surfaced so the label text/tests can read the
	 *  precise state (lt1h vs imminent) even though the VISUAL collapses them. */
	ttlBucket: TtlState;
	/** The three-tier visual the dash/glyph/alert are keyed off. */
	ttlVisual: TtlVisual;
}

// ── Mass → thickness + colour (spec §2) ──────────────────────────────────────
// Only the colour token's VALUE differs per palette; the var name is the same,
// so the component just toggles a palette class/attribute on a wrapper and the
// cascade picks the right hex. Widths never change.
const MASS_WIDTH: Record<Mass, number> = {
	fresh: 5,
	half: 3,
	critical: 2 // floored at 2, not 1.5 (spec §2)
};

const MASS_COLOUR_VAR: Record<Mass, string> = {
	fresh: 'var(--mass-fresh)',
	half: 'var(--mass-half)',
	critical: 'var(--mass-critical)'
};

/** Resolve the mass channels. Palette is irrelevant to width and only renames
 *  nothing (the var is constant) — it's passed for symmetry / future use. */
export function resolveMass(mass: Mass): MassEncoding {
	return { width: MASS_WIDTH[mass], colourVar: MASS_COLOUR_VAR[mass] };
}

/**
 * Resolve the derived alert layer from the TTL visual alone, over the THREE
 * visual tiers:
 *   above 4 h        → none      (calm)
 *   < 4 h            → warning   (amber, gentle breath)
 *   < 1 h OR imminent → critical (red danger, deep breath) ← the loud signal
 *
 * The glow is PURE TTL — mass plays no part. Critical MASS is already conveyed by
 * the thin red line (mass owns thickness + colour); adding a glow for it as well
 * was redundant and made a time-stable crit-mass hole sit there with a static,
 * non-pulsing halo that just read as "broken". So mass-critical with healthy time
 * gets NO casing; the halo is reserved entirely for the time axis, which keeps
 * motion rare and meaningful.
 */
export function resolveAlert(visual: TtlVisual): AlertEncoding {
	if (visual === 'critical') {
		return {
			level: 'danger',
			// The casing uses the richer halo red (not the vermillion line/glyph tint)
			// so the glow actually reads as an alarm against the dark canvas.
			casingColourVar: 'var(--alert-danger-halo)',
			// MAX (peak) width — large, so a frozen (reduced-motion) critical reads as
			// the loudest state AND is clearly bigger than a frozen warning.
			casingWidth: 26,
			casingOpacity: 0.5,
			breatheClass: 'halo-red'
		};
	}
	if (visual === 'warning') {
		return {
			level: 'warning',
			casingColourVar: 'var(--alert-warning)',
			// MAX (peak) width — distinctly SMALLER than critical's, so size alone
			// separates the two when frozen.
			casingWidth: 16,
			casingOpacity: 0.3,
			breatheClass: 'halo-amber'
		};
	}
	return {
		level: 'none',
		casingColourVar: 'transparent',
		casingWidth: 0,
		casingOpacity: 0,
		breatheClass: ''
	};
}

/** The single entry point: resolve a connection's raw mass + remaining-minutes
 *  into every channel. `palette` is accepted for the API the spec describes; the
 *  hue swap itself is done in CSS via a palette attribute, so it does not change
 *  the resolved var names here. */
export function resolveEdgeEncoding(
	mass: Mass,
	ttlRemainingMin: number,
	_palette: Palette = 'standard'
): EdgeEncoding {
	const ttlBucket = ttlState(ttlRemainingMin);
	const visual = ttlVisual(ttlBucket);
	return {
		mass: resolveMass(mass),
		alert: resolveAlert(visual),
		ttlBucket,
		ttlVisual: visual
	};
}
