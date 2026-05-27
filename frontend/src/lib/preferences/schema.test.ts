import { describe, it, expect } from 'vitest';
import {
	DEFAULT_PREFERENCES,
	TEXT_SIZE_PERCENT,
	activeOverrides,
	coercePreferences,
	isPreferenceKey,
	isValidValue,
	type Preferences
} from './schema';

describe('schema validation helpers', () => {
	it('isPreferenceKey recognises known keys and rejects others', () => {
		expect(isPreferenceKey('text_size')).toBe(true);
		expect(isPreferenceKey('reduce_motion')).toBe(true);
		expect(isPreferenceKey('locale')).toBe(true);
		expect(isPreferenceKey('nonsense')).toBe(false);
	});

	it('isValidValue accepts allowed values and rejects invalid ones', () => {
		expect(isValidValue('text_size', 'large')).toBe(true);
		expect(isValidValue('text_size', 'enormous')).toBe(false);
		expect(isValidValue('reduce_motion', 'on')).toBe(true);
		expect(isValidValue('large_targets', 'auto')).toBe(false); // toggle has no auto
		expect(isValidValue('locale', 'en')).toBe(true);
		expect(isValidValue('locale', 'de')).toBe(true);
		expect(isValidValue('locale', 'martian')).toBe(false);
	});

	it('auto/regular text size leaves the root font-size unset', () => {
		expect(TEXT_SIZE_PERCENT.auto).toBeNull();
		expect(TEXT_SIZE_PERCENT.regular).toBe(100);
		expect(TEXT_SIZE_PERCENT.large).toBeGreaterThan(100);
		expect(TEXT_SIZE_PERCENT.small).toBeLessThan(100);
	});
});

describe('coercePreferences', () => {
	it('returns defaults for empty / non-object input', () => {
		expect(coercePreferences(null)).toEqual(DEFAULT_PREFERENCES);
		expect(coercePreferences('nope')).toEqual(DEFAULT_PREFERENCES);
		expect(coercePreferences({})).toEqual(DEFAULT_PREFERENCES);
	});

	it('keeps valid keys and ignores invalid values', () => {
		const result = coercePreferences({ text_size: 'large', reduce_motion: 'bogus' });
		expect(result.text_size).toBe('large');
		expect(result.reduce_motion).toBe('auto'); // invalid → default
	});

	it('keeps a valid locale and defaults an invalid one', () => {
		expect(coercePreferences({ locale: 'en' }).locale).toBe('en');
		expect(coercePreferences({ locale: 'martian' }).locale).toBe('en'); // invalid → default
	});

	it('ignores genuinely foreign keys (forward-compatible)', () => {
		const result = coercePreferences({ future_feature: 'x', high_contrast: 'on' });
		expect(result.high_contrast).toBe('on');
		expect(result).not.toHaveProperty('future_feature');
	});
});

describe('activeOverrides', () => {
	it('returns only keys that differ from defaults', () => {
		const prefs: Preferences = {
			...DEFAULT_PREFERENCES,
			text_size: 'large',
			dyslexia_font: 'on'
		};
		expect(activeOverrides(prefs)).toEqual({ text_size: 'large', dyslexia_font: 'on' });
	});

	it('returns an empty object when all values are default', () => {
		expect(activeOverrides({ ...DEFAULT_PREFERENCES })).toEqual({});
	});
});
