// Accessibility preference schema — the single source of truth for keys, their
// allowed values, and their defaults.
//
// keep in sync with: backend/src/dto/preferences.rs
//
// This is the generic preference substrate. Other features add their own keys
// here — e.g. add-internationalisation-support adds `locale`, reusing this
// store, the /preferences proxy, and the no-FOUC bootstrap (it sets <html lang>
// the same way text_size sets html font-size). No new store or endpoint needed.

export type TextSize = 'auto' | 'small' | 'regular' | 'large';
export type TriState = 'auto' | 'on' | 'off';
export type Toggle = 'off' | 'on';

export interface Preferences {
	text_size: TextSize;
	reduce_motion: TriState;
	high_contrast: TriState;
	large_targets: Toggle;
	dyslexia_font: Toggle;
}

export type PreferenceKey = keyof Preferences;

/** A partial update — only the present keys are changed. */
export type PreferencesPatch = Partial<Preferences>;

/** Allowed values per key — used to validate values read from localStorage / the server. */
export const ALLOWED_VALUES: { [K in PreferenceKey]: ReadonlyArray<Preferences[K]> } = {
	text_size: ['auto', 'small', 'regular', 'large'],
	reduce_motion: ['auto', 'on', 'off'],
	high_contrast: ['auto', 'on', 'off'],
	large_targets: ['off', 'on'],
	dyslexia_font: ['off', 'on']
};

/** The defaults — every key resolves to these when unset. `auto`/`off` means "no override". */
export const DEFAULT_PREFERENCES: Preferences = {
	text_size: 'auto',
	reduce_motion: 'auto',
	high_contrast: 'auto',
	large_targets: 'off',
	dyslexia_font: 'off'
};

/** localStorage key under which the preference bag is persisted. */
export const STORAGE_KEY = 'erbridge:preferences';

/** Text-size → root font-size percentage. `auto`/`regular` leave the browser default. */
export const TEXT_SIZE_PERCENT: Record<TextSize, number | null> = {
	auto: null,
	regular: 100,
	small: 87.5,
	large: 125
};

/** True when `key` is a recognised preference key. */
export function isPreferenceKey(key: string): key is PreferenceKey {
	return key in DEFAULT_PREFERENCES;
}

/** True when `value` is allowed for `key`. */
export function isValidValue<K extends PreferenceKey>(key: K, value: unknown): value is Preferences[K] {
	return (ALLOWED_VALUES[key] as ReadonlyArray<unknown>).includes(value);
}

/**
 * Coerce an arbitrary parsed object into a complete `Preferences`, keeping only
 * recognised keys with valid values and falling back to the default otherwise.
 * Tolerates extra/foreign keys (e.g. a future `locale`) by ignoring them.
 */
export function coercePreferences(raw: unknown): Preferences {
	const result: Preferences = { ...DEFAULT_PREFERENCES };
	if (raw && typeof raw === 'object') {
		for (const key of Object.keys(DEFAULT_PREFERENCES) as PreferenceKey[]) {
			const value = (raw as Record<string, unknown>)[key];
			if (isValidValue(key, value)) {
				// Safe: isValidValue narrows value to Preferences[key].
				(result[key] as Preferences[PreferenceKey]) = value;
			}
		}
	}
	return result;
}

/** The subset of `prefs` whose values differ from the defaults — i.e. active overrides. */
export function activeOverrides(prefs: Preferences): Partial<Preferences> {
	const out: Partial<Preferences> = {};
	for (const key of Object.keys(DEFAULT_PREFERENCES) as PreferenceKey[]) {
		if (prefs[key] !== DEFAULT_PREFERENCES[key]) {
			(out[key] as Preferences[PreferenceKey]) = prefs[key];
		}
	}
	return out;
}
