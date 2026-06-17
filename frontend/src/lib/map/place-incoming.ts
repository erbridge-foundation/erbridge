/**
 * Incremental placement — where a newly-arrived node lands.
 *
 * The map is laid out ONCE on initial load (`layoutSeed`). After that the graph
 * only changes through discrete SSE events (see `MapEvent`), and an added node
 * must be placed WITHOUT re-laying-out the whole map: we drop it one flow-step
 * out from its anchor (the existing system it arrived through), in the map's
 * current direction, then the caller resolves collisions across the graph so the
 * new node "ripples" its neighbours apart if it landed on one.
 *
 * This is the live-placement counterpart to `layoutSeed`'s one-shot seed. It is
 * PURE: (anchor position, direction) → ideal XY. The anchor must already have a
 * position (it's an existing node); if it somehow doesn't, we fall back to the
 * origin so the node is still placed (and collision-resolution spreads it).
 */

import type { LayoutDirection, XY } from './types';

// One flow-step. Matches the rank/sibling spacing in `layout.ts` so an added
// node sits a column/row out from its anchor, consistent with the initial seed.
const STEP_X = 260;
const STEP_Y = 150;
const STEP_R = 220;

/**
 * The ideal slot for a node arriving through `anchor`, one step along the flow
 * direction:
 *   LR → +x   RL → -x   TB → +y   BT → -y
 *   radial → pushed outward from the origin along the anchor's bearing (a node
 *            with no bearing, i.e. the anchor at the origin, steps +x).
 */
export function placeIncoming(anchor: XY, dir: LayoutDirection): XY {
	switch (dir) {
		case 'LR':
			return { x: anchor.x + STEP_X, y: anchor.y };
		case 'RL':
			return { x: anchor.x - STEP_X, y: anchor.y };
		case 'TB':
			return { x: anchor.x, y: anchor.y + STEP_Y };
		case 'BT':
			return { x: anchor.x, y: anchor.y - STEP_Y };
		case 'radial': {
			// Step outward from the origin along the anchor's bearing. At the origin
			// there is no bearing — step +x so the node is still placed off-anchor.
			const len = Math.hypot(anchor.x, anchor.y);
			if (len === 0) return { x: STEP_R, y: 0 };
			const ux = anchor.x / len;
			const uy = anchor.y / len;
			return { x: Math.round(anchor.x + ux * STEP_R), y: Math.round(anchor.y + uy * STEP_R) };
		}
	}
}
