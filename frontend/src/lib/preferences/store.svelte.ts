// The preferences store: localStorage-first, applied to <html>, with optional
// backend sync for authenticated users.
//
//   localStorage  ── source of truth at the edge (anonymous, instant, no FOUC)
//        ⇅
//   backend       ── cross-device durability for authenticated users
//
// Reconciliation on authenticated init: if the server holds any override the
// server wins; otherwise, if localStorage holds overrides, push them up
// (preserving an anonymous user's setup on first login).

import { browser } from '$app/environment';
import { getLocale, setLocale } from '$lib/paraglide/runtime';
import { applyPreferences } from './apply';
import {
	DEFAULT_PREFERENCES,
	STORAGE_KEY,
	activeOverrides,
	coercePreferences,
	type Locale,
	type Preferences,
	type PreferencesPatch
} from './schema';

/**
 * Bridge the stored locale to Paraglide's PARAGLIDE_LOCALE cookie — the single
 * integration point between the preference substrate and the i18n library.
 * Paraglide resolves the active locale from that cookie during SSR, so writing
 * it keeps the server-rendered language in step with the stored preference.
 *
 * `reload` controls whether the page re-renders in the new locale immediately:
 *  - true  on commit/reset (the user changed locale; re-render so the new
 *           language takes effect with no wrong-language flash). setLocale only
 *           reloads when the locale actually differs from the current one.
 *  - false on reconcile (a routine page load; sync the cookie silently — never
 *           trigger a reload during init).
 */
function bridgeLocale(locale: Locale, reload: boolean): void {
	if (!browser) return;
	let current: string | undefined;
	try {
		current = getLocale();
	} catch {
		// No locale resolved yet — treat as a change so the cookie gets written.
	}
	if (current === locale) return;
	setLocale(locale, { reload });
}

interface PrefState {
	current: Preferences;
	/** True once the store has reconciled with the server (or determined no session). */
	synced: boolean;
}

const state = $state<PrefState>({
	current: { ...DEFAULT_PREFERENCES },
	synced: false
});

/** Read the persisted preference set from localStorage (defaults if absent/corrupt). */
function readLocal(): Preferences {
	if (!browser) return { ...DEFAULT_PREFERENCES };
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return { ...DEFAULT_PREFERENCES };
		return coercePreferences(JSON.parse(raw));
	} catch {
		return { ...DEFAULT_PREFERENCES };
	}
}

/** Persist the preference set to localStorage. */
function writeLocal(prefs: Preferences): void {
	if (!browser) return;
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
	} catch {
		// Storage full / disabled — the in-memory + DOM state still applies for the session.
	}
}

/** True when a preference set differs from the defaults in at least one key. */
function hasOverrides(prefs: Preferences): boolean {
	return Object.keys(activeOverrides(prefs)).length > 0;
}

/** PATCH the backend via the SvelteKit proxy; returns the merged set or null on failure. */
async function syncToServer(patch: PreferencesPatch): Promise<Preferences | null> {
	if (!browser) return null;
	try {
		const res = await fetch('/preferences', {
			method: 'PATCH',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(patch)
		});
		if (!res.ok) return null;
		const body = await res.json();
		return coercePreferences(body.data);
	} catch {
		return null;
	}
}

export const preferences = {
	get current(): Preferences {
		return state.current;
	},
	get synced(): boolean {
		return state.synced;
	},

	/**
	 * The persisted preference set (from localStorage), independent of any live
	 * preview held in `current`. The /preferences page diffs its staged set
	 * against this to decide the dirty/clean state.
	 */
	get persisted(): Preferences {
		return readLocal();
	},

	/**
	 * Hydrate from localStorage and apply to the DOM. Call early (e.g. in the root
	 * layout) — the inline app.html script has already applied the same values
	 * before paint, so this does not re-flash.
	 */
	hydrate(): void {
		state.current = readLocal();
		if (browser) applyPreferences(state.current);
	},

	/**
	 * Reconcile localStorage with the authenticated account's server preferences.
	 * `serverPrefs` is null for anonymous users (no reconciliation, no sync).
	 */
	async reconcile(serverPrefs: Preferences | null): Promise<void> {
		const local = readLocal();

		if (!serverPrefs) {
			// Anonymous: localStorage is authoritative; nothing to sync.
			state.current = local;
			state.synced = true;
			if (browser) applyPreferences(state.current);
			return;
		}

		if (hasOverrides(serverPrefs)) {
			// Server has explicit choices — server wins, overwrite localStorage.
			state.current = serverPrefs;
			writeLocal(serverPrefs);
		} else if (hasOverrides(local)) {
			// Server is all-default but the user configured prefs while anonymous —
			// push local up on first login.
			const pushed = await syncToServer(activeOverrides(local));
			state.current = pushed ?? local;
			writeLocal(state.current);
		} else {
			// Both at defaults.
			state.current = serverPrefs;
		}

		state.synced = true;
		if (browser) applyPreferences(state.current);
		// Sync Paraglide's cookie to the reconciled locale without reloading —
		// this runs on every authenticated load, so a reload here would loop.
		bridgeLocale(state.current.locale, false);
	},

	/**
	 * Apply a patch as a *preview* only — update in-memory state and the DOM, but
	 * do NOT persist. The /preferences page uses this to stage changes live; they
	 * persist only on Apply (commit) and are dropped on Discard / navigate-away.
	 */
	preview(patch: PreferencesPatch): void {
		state.current = { ...state.current, ...patch };
		if (browser) applyPreferences(state.current);
	},

	/**
	 * Commit a patch: update state, apply to the DOM, persist to localStorage, and
	 * (for authenticated users) sync to the backend.
	 */
	async commit(patch: PreferencesPatch): Promise<void> {
		state.current = { ...state.current, ...patch };
		if (browser) applyPreferences(state.current);
		writeLocal(state.current);
		// Persist to the backend before bridging the locale: bridgeLocale with
		// reload:true reloads the page when the locale changed, which would abort
		// an in-flight sync. Awaiting first guarantees the PATCH completes.
		await syncToServer(patch);
		bridgeLocale(state.current.locale, true);
	},

	/** Re-apply the persisted set to the DOM, discarding any unpersisted preview. */
	revertToPersisted(): void {
		state.current = readLocal();
		if (browser) applyPreferences(state.current);
	},

	/**
	 * Reset every preference to its default, apply to the DOM, persist, and sync.
	 * The recovery action: always available on /preferences so a user whose
	 * applied setting broke another page can return and restore a working state.
	 * Syncs the full default set (not just a diff) so any prior server overrides
	 * are explicitly overwritten back to default.
	 */
	async resetToDefaults(): Promise<void> {
		state.current = { ...DEFAULT_PREFERENCES };
		if (browser) applyPreferences(state.current);
		writeLocal(state.current);
		await syncToServer({ ...DEFAULT_PREFERENCES });
		bridgeLocale(state.current.locale, true);
	}
};
