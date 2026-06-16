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

// jsdom implements neither matchMedia nor ResizeObserver, both of which
// @xyflow/svelte (Svelte Flow) touches at mount. Stub them so the map canvas
// custom node/edge component tests can mount a real <SvelteFlow>.
if (typeof window !== 'undefined' && !window.matchMedia) {
	window.matchMedia = (query: string): MediaQueryList =>
		({
			matches: false,
			media: query,
			onchange: null,
			addListener() {},
			removeListener() {},
			addEventListener() {},
			removeEventListener() {},
			dispatchEvent() {
				return false;
			}
		}) as MediaQueryList;
}

if (typeof globalThis !== 'undefined' && !('ResizeObserver' in globalThis)) {
	class ResizeObserverStub {
		observe() {}
		unobserve() {}
		disconnect() {}
	}
	// @ts-expect-error — minimal ResizeObserver shim for jsdom.
	globalThis.ResizeObserver = ResizeObserverStub;
}
