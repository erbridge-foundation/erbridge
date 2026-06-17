import { describe, it, expect } from 'vitest';
import { placeIncoming } from './place-incoming';
import type { XY } from './types';

const anchor: XY = { x: 100, y: 50 };

describe('placeIncoming', () => {
	it('LR steps +x from the anchor, same y', () => {
		const p = placeIncoming(anchor, 'LR');
		expect(p.x).toBeGreaterThan(anchor.x);
		expect(p.y).toBe(anchor.y);
	});

	it('RL steps -x from the anchor, same y', () => {
		const p = placeIncoming(anchor, 'RL');
		expect(p.x).toBeLessThan(anchor.x);
		expect(p.y).toBe(anchor.y);
	});

	it('TB steps +y from the anchor, same x', () => {
		const p = placeIncoming(anchor, 'TB');
		expect(p.y).toBeGreaterThan(anchor.y);
		expect(p.x).toBe(anchor.x);
	});

	it('BT steps -y from the anchor, same x', () => {
		const p = placeIncoming(anchor, 'BT');
		expect(p.y).toBeLessThan(anchor.y);
		expect(p.x).toBe(anchor.x);
	});

	it('radial pushes outward from the origin (farther than the anchor)', () => {
		const p = placeIncoming(anchor, 'radial');
		expect(Math.hypot(p.x, p.y)).toBeGreaterThan(Math.hypot(anchor.x, anchor.y));
	});

	it('radial at the origin still steps off-anchor (no NaN from a zero bearing)', () => {
		const p = placeIncoming({ x: 0, y: 0 }, 'radial');
		expect(Number.isFinite(p.x)).toBe(true);
		expect(Number.isFinite(p.y)).toBe(true);
		expect(Math.hypot(p.x, p.y)).toBeGreaterThan(0);
	});

	it('is pure — does not mutate the anchor', () => {
		const a: XY = { x: 5, y: 7 };
		placeIncoming(a, 'LR');
		expect(a).toEqual({ x: 5, y: 7 });
	});
});
