/**
 * Placement persistence — Fork 1, sandbox backend.
 *
 * Manual node positions ("nudges") survive a session restart. The sandbox store
 * is `localStorage`, keyed by map; the saved shape is `{ [tabId]: Positions }`
 * because each tab carries its own layout/placement. Placement is a personal
 * convenience cache — NEVER graph truth.
 *
 * This module is a deliberately THIN seam: the real backend (localStorage vs a
 * per-user server store) is a Track-2 decision, so swapping the store later is a
 * one-module change. Everything above it (reconcile, MapCanvas) talks to
 * `load` / `save` / `clearTab` and knows nothing about localStorage.
 */

import type { Positions } from './types';

type Saved = Record<string, Positions>;

const PREFIX = 'erbridge:map-placement:';
const key = (mapId: string) => `${PREFIX}${mapId}`;

/** SSR / test guard — no `window` ⇒ nothing persisted, an empty store. */
function store(): Storage | null {
	return typeof localStorage !== 'undefined' ? localStorage : null;
}

/** Whole-map saved placement: `{ [tabId]: { [systemId]: {x,y} } }`. */
export function load(mapId: string): Saved {
	const s = store();
	if (!s) return {};
	const raw = s.getItem(key(mapId));
	if (!raw) return {};
	try {
		const parsed: unknown = JSON.parse(raw);
		// Defensive: a corrupt blob shouldn't take the canvas down — treat as empty.
		return parsed && typeof parsed === 'object' ? (parsed as Saved) : {};
	} catch {
		return {};
	}
}

/** Saved placement for a single tab (the unit the canvas reads). */
export function loadTab(mapId: string, tabId: string): Positions {
	return load(mapId)[tabId] ?? {};
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * Persist a tab's positions. Debounced (drag fires many move events; we don't
 * write on every tick — see the sveltekit-node Svelte-Flow guidance). Pass
 * `debounceMs: 0` (tests) to write synchronously.
 */
export function save(mapId: string, tabId: string, positions: Positions, debounceMs = 400): void {
	const s = store();
	if (!s) return;
	const commit = () => {
		const all = load(mapId);
		all[tabId] = positions;
		s.setItem(key(mapId), JSON.stringify(all));
	};
	if (debounceMs <= 0) {
		commit();
		return;
	}
	clearTimeout(saveTimer);
	saveTimer = setTimeout(commit, debounceMs);
}

/** Forget a tab's saved placement (the "redo layout" action clears it so the
 *  next render falls back to the fresh layout seed). */
export function clearTab(mapId: string, tabId: string): void {
	const s = store();
	if (!s) return;
	const all = load(mapId);
	if (tabId in all) {
		delete all[tabId];
		s.setItem(key(mapId), JSON.stringify(all));
	}
}
