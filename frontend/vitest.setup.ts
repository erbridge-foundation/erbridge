import '@testing-library/jest-dom/vitest';

// jsdom does not implement Element.animate (used by Svelte's transition
// engine). Polyfill with a no-op that returns a minimal Animation-shaped
// object so transitions don't crash in component tests.
if (typeof Element !== 'undefined' && !Element.prototype.animate) {
	// @ts-expect-error — minimal Animation shim for jsdom; the transition engine
	// only inspects .finished / cancel() / play(), which the stub satisfies.
	Element.prototype.animate = function animateStub() {
		return {
			finished: Promise.resolve(),
			cancel() {},
			play() {},
			pause() {},
			finish() {},
			reverse() {},
			addEventListener() {},
			removeEventListener() {},
			currentTime: 0,
			playState: 'finished',
			effect: null
		};
	};
}
