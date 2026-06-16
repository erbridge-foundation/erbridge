import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import StatusIcon from './StatusIcon.svelte';

afterEach(() => cleanup());

// The glyph silhouettes are what make the levels distinguishable without colour,
// so the tests assert on the rendered shape primitives, not just a level attr.
function svg(container: HTMLElement) {
	return container.querySelector('svg');
}

// The enclosing shape (filled in the level colour) is what makes the silhouette
// distinct: a circle for ok/error, a triangle path for warning. The inner glyph
// is a stroked `.mark` path punched out in a contrast colour.
function enclosingIsCircle(el: SVGElement): boolean {
	// A large filled circle is the enclosing shape; the warning's bang-dot is a
	// tiny `.mark-fill` circle, so require radius >= 5 to ignore it.
	const circles = Array.from(el.querySelectorAll('circle'));
	return circles.some((c) => Number(c.getAttribute('r')) >= 5);
}
function markPath(el: SVGElement): string {
	return el.querySelector('.mark')!.getAttribute('d') ?? '';
}

describe('StatusIcon glyphs', () => {
	it('ok renders a check inside a solid circle', () => {
		const { container } = render(StatusIcon, { props: { level: 'ok' } });
		const el = svg(container)!;
		expect(enclosingIsCircle(el)).toBe(true); // round silhouette
		expect(el.querySelector('.mark')).not.toBeNull(); // the check glyph
		expect(container.querySelector('[data-level="ok"]')).not.toBeNull();
	});

	it('error renders a cross inside a solid circle', () => {
		const { container } = render(StatusIcon, { props: { level: 'error' } });
		const el = svg(container)!;
		expect(enclosingIsCircle(el)).toBe(true); // round silhouette like ok
		expect(container.querySelector('[data-level="error"]')).not.toBeNull();
	});

	it('warning renders a bang inside a triangle (no enclosing circle)', () => {
		const { container } = render(StatusIcon, { props: { level: 'warning' } });
		const el = svg(container)!;
		// Pointed silhouette: a closed triangle path, and crucially NOT an
		// enclosing circle like ok/error.
		expect(enclosingIsCircle(el)).toBe(false);
		const triangle = el.querySelector('path:not(.mark)')!;
		expect(triangle.getAttribute('d')).toMatch(/z$/i); // closes back to start
		expect(container.querySelector('[data-level="warning"]')).not.toBeNull();
	});

	// Collect each level's inner mark path so we can assert they are mutually
	// distinct. One render per test keeps Svelte's effect context happy.
	const marks: Record<string, string> = {};
	for (const level of ['ok', 'warning', 'error'] as const) {
		it(`captures a distinct glyph mark for ${level}`, () => {
			const { container } = render(StatusIcon, { props: { level } });
			marks[level] = markPath(svg(container)!);
			expect(marks[level]).not.toBe('');
		});
	}

	it('the three glyph marks are mutually distinct', () => {
		const values = Object.values(marks);
		expect(new Set(values).size).toBe(values.length);
	});
});

describe('StatusIcon accessibility modes', () => {
	it('is decorative when no tooltip: a non-focusable, aria-hidden span', () => {
		const { container } = render(StatusIcon, { props: { level: 'ok' } });
		const wrapper = container.querySelector('.status-icon')!;
		expect(wrapper.tagName).toBe('SPAN');
		expect(wrapper.getAttribute('aria-hidden')).toBe('true');
		expect(wrapper.hasAttribute('tabindex')).toBe(false);
		// not a button → not in the tab order / not interactive
		expect(container.querySelector('button')).toBeNull();
	});

	it('exposes a tooltip accessibly when given: a focusable button, named, with aria-describedby (not a bare title)', () => {
		const { container } = render(StatusIcon, {
			props: { level: 'warning', tooltip: 'Token transferred' }
		});
		const wrapper = container.querySelector('.status-icon')!;

		// a real <button> is natively focusable + keyboard-operable
		expect(wrapper.tagName).toBe('BUTTON');
		expect(wrapper.getAttribute('aria-label')).toBe('Token transferred');
		expect(wrapper.getAttribute('aria-hidden')).toBeNull();

		// described-by points at a real tooltip element, not a bare title attribute
		expect(wrapper.hasAttribute('title')).toBe(false);
		const describedBy = wrapper.getAttribute('aria-describedby');
		expect(describedBy).toBeTruthy();
		const tip = container.querySelector(`#${describedBy}`);
		expect(tip).not.toBeNull();
		expect(tip!.textContent).toBe('Token transferred');
	});
});
