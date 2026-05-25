// Applies a preference set to <html>. The single place that maps preference
// values onto the DOM, used by the runtime store. The no-FOUC inline bootstrap
// in app.html mirrors this logic in plain JS (it cannot import this module
// before hydration), so keep the two in sync.

import { TEXT_SIZE_PERCENT, type Preferences } from './schema';

/**
 * Apply `prefs` to the given root element (defaults to `document.documentElement`):
 *  - `text_size` sets `font-size` (a percentage) or clears it for `auto`.
 *  - the tri-state / toggle prefs set a `data-*` attribute only when overriding
 *    a default; `auto`/`off` remove the attribute so the CSS `@media` default wins.
 */
export function applyPreferences(prefs: Preferences, root: HTMLElement = document.documentElement): void {
	const percent = TEXT_SIZE_PERCENT[prefs.text_size];
	if (percent === null) {
		root.style.removeProperty('font-size');
	} else {
		root.style.fontSize = `${percent}%`;
	}

	setOverrideAttr(root, 'data-reduce-motion', prefs.reduce_motion, 'auto');
	setOverrideAttr(root, 'data-high-contrast', prefs.high_contrast, 'auto');
	setOverrideAttr(root, 'data-large-targets', prefs.large_targets, 'off');
	setOverrideAttr(root, 'data-dyslexia-font', prefs.dyslexia_font, 'off');
}

/** Set `attr` to `value`, or remove it when `value` is the no-override default. */
function setOverrideAttr(root: HTMLElement, attr: string, value: string, defaultValue: string): void {
	if (value === defaultValue) {
		root.removeAttribute(attr);
	} else {
		root.setAttribute(attr, value);
	}
}
