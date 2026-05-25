import { describe, it, expect, beforeEach } from 'vitest';
import { applyPreferences } from './apply';
import { DEFAULT_PREFERENCES, type Preferences } from './schema';

function root(): HTMLElement {
	return document.documentElement;
}

describe('applyPreferences', () => {
	beforeEach(() => {
		const el = root();
		el.removeAttribute('style');
		for (const attr of ['data-reduce-motion', 'data-high-contrast', 'data-large-targets', 'data-dyslexia-font']) {
			el.removeAttribute(attr);
		}
	});

	it('leaves font-size unset for auto text size', () => {
		applyPreferences({ ...DEFAULT_PREFERENCES, text_size: 'auto' });
		expect(root().style.fontSize).toBe('');
	});

	it('sets a font-size percentage for an explicit text size', () => {
		applyPreferences({ ...DEFAULT_PREFERENCES, text_size: 'large' });
		expect(root().style.fontSize).toBe('125%');
	});

	it('does NOT set data attributes for auto/off (lets the @media default win)', () => {
		applyPreferences({ ...DEFAULT_PREFERENCES });
		expect(root().hasAttribute('data-reduce-motion')).toBe(false);
		expect(root().hasAttribute('data-high-contrast')).toBe(false);
		expect(root().hasAttribute('data-large-targets')).toBe(false);
		expect(root().hasAttribute('data-dyslexia-font')).toBe(false);
	});

	it('sets data attributes only for explicit overrides', () => {
		const prefs: Preferences = {
			text_size: 'auto',
			reduce_motion: 'on',
			high_contrast: 'off',
			large_targets: 'on',
			dyslexia_font: 'off'
		};
		applyPreferences(prefs);
		expect(root().getAttribute('data-reduce-motion')).toBe('on');
		expect(root().getAttribute('data-high-contrast')).toBe('off');
		expect(root().getAttribute('data-large-targets')).toBe('on');
		// dyslexia_font is 'off' (default) → no attribute
		expect(root().hasAttribute('data-dyslexia-font')).toBe(false);
	});

	it('clears a previously-set override when re-applied at default', () => {
		applyPreferences({ ...DEFAULT_PREFERENCES, reduce_motion: 'on' });
		expect(root().getAttribute('data-reduce-motion')).toBe('on');
		applyPreferences({ ...DEFAULT_PREFERENCES, reduce_motion: 'auto' });
		expect(root().hasAttribute('data-reduce-motion')).toBe(false);
	});
});
