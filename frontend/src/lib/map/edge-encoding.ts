/**
 * The ONE config object that resolves a connection's two independent variables
 * (mass + TTL) plus the derived alert into every visual channel of an edge —
 * see the wormhole edge-encoding spec. Pure + framework-free so it is unit-
 * testable without SvelteFlow's edge pipeline (which needs measured node sizes
 * jsdom can't supply); ConnectionEdge.svelte is then thin plumbing over this.
 *
 * Channel ownership (the spec's key principle — separate READING from ALERTING):
 *   MASS  → line thickness + colour.        (a thin red line is its own alarm)
 *   TTL   → dash pattern + a single glyph.   (hue-independent + an unambiguous tiebreaker)
 *   ALERT → casing (halo) + filled badge.    (PURE TTL; owns the "loudest" cue)
 *
 * Mass never gets a glyph (avoids two icons fighting on a crit-mass + EOL edge);
 * the glyph is TTL-owned. A fresh + stable edge fires NOTHING and stays calm.
 */

import type { Mass, TtlState, TtlVisual } from './types';
import { ttlState, ttlVisual } from './types';

/** Standard vs colour-blind. The ONLY thing it swaps is the three mass hues —
 *  thickness, dash, glyph, motion, and the alert layer are identical across
 *  palettes (so swapping is a one-line change). */
export type Palette = 'standard' | 'colourblind';

/** The TTL glyph escalates clock → octagon across the three VISUAL tiers (calm
 *  shows none, warning a clock, critical an octagon); the triangle is retained in
 *  the union for callers but the three-tier visual doesn't emit it. The edge
 *  component maps these names to an inline SVG (shape-distinct, not colour-only),
 *  echoing the StatusIcon approach. */
export type TtlGlyph = 'none' | 'clock' | 'triangle' | 'octagon';

/** Resolved mass channels: how wide + what colour the main line is. */
export interface MassEncoding {
	/** Stroke width in px. Critical is floored at 2 so the imminent dash-dot
	 *  doesn't collapse to solid on the thinnest line (spec §2). */
	width: number;
	/** A CSS custom-property reference; the actual hex lives in the palette tokens
	 *  (app.css) so a palette swap is a token swap, not a recompute here. */
	colourVar: string;
}

/** Resolved TTL channels: dash pattern + which glyph + whether/how it breathes. */
export interface TtlEncoding {
	/** SVG `stroke-dasharray` value, or `''` for a solid line (stable). */
	dashArray: string;
	glyph: TtlGlyph;
	/** Tint token for the glyph (neutral / warning / danger). */
	glyphColourVar: string;
}

/** The derived alert layer — the casing (under-stroke halo) + label badge that
 *  own "attention". Fires on the TTL visual ALONE (mass plays no part — see
 *  resolveAlert); `level === 'none'` is calm. */
export interface AlertEncoding {
	level: 'none' | 'warning' | 'danger';
	/** Casing colour token (only meaningful when level !== 'none'). */
	casingColourVar: string;
	/** Casing base width / opacity (the resting state; the breathing animation
	 *  swells around these — see the .halo-* CSS in ConnectionEdge). */
	casingWidth: number;
	casingOpacity: number;
	/** Which breathing keyframe class to apply, or `''` for no motion. Under
	 *  prefers-reduced-motion the component drops the class and renders the casing
	 *  static at the breath midpoint instead. */
	breatheClass: '' | 'halo-amber' | 'halo-red';
}

/** Everything an edge needs to render, resolved from the two raw variables. */
export interface EdgeEncoding {
	mass: MassEncoding;
	ttl: TtlEncoding;
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

// ── TTL → dash + glyph, keyed off the THREE visual tiers ──────────────────────
// The four TTL states collapse to three visuals (see TtlVisual): lt1h + imminent
// share the critical visual. Dash gaps are kept wide enough to survive round
// line-caps (which extend each dash by ½ stroke-width on both ends — the reason
// dense dashes read solid).
const TTL_DASH: Record<TtlVisual, string> = {
	calm: '',
	warning: '14 8',
	critical: '9 9 2 9' // dash-dot — the loud "act now / too late" texture
};

const TTL_GLYPH: Record<TtlVisual, TtlGlyph> = {
	calm: 'none',
	warning: 'clock',
	critical: 'octagon'
};

const TTL_GLYPH_COLOUR_VAR: Record<TtlVisual, string> = {
	calm: 'transparent',
	warning: 'var(--alert-warning)',
	critical: 'var(--alert-danger)'
};

/** Resolve the mass channels. Palette is irrelevant to width and only renames
 *  nothing (the var is constant) — it's passed for symmetry / future use. */
export function resolveMass(mass: Mass): MassEncoding {
	return { width: MASS_WIDTH[mass], colourVar: MASS_COLOUR_VAR[mass] };
}

/** Resolve the TTL dash + glyph from the three-tier visual. */
export function resolveTtl(visual: TtlVisual): TtlEncoding {
	return {
		dashArray: TTL_DASH[visual],
		glyph: TTL_GLYPH[visual],
		glyphColourVar: TTL_GLYPH_COLOUR_VAR[visual]
	};
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
			casingWidth: 13,
			casingOpacity: 0.28,
			breatheClass: 'halo-red'
		};
	}
	if (visual === 'warning') {
		return {
			level: 'warning',
			casingColourVar: 'var(--alert-warning)',
			casingWidth: 11,
			casingOpacity: 0.18,
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
		ttl: resolveTtl(visual),
		alert: resolveAlert(visual),
		ttlBucket,
		ttlVisual: visual
	};
}
