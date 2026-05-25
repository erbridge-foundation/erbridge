import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { preferences } from './store.svelte';
import { DEFAULT_PREFERENCES, STORAGE_KEY, type Preferences } from './schema';

function withOverrides(o: Partial<Preferences>): Preferences {
	return { ...DEFAULT_PREFERENCES, ...o };
}

describe('preferences store', () => {
	beforeEach(() => {
		localStorage.clear();
		preferences.revertToPersisted(); // reset in-memory state to defaults
		vi.restoreAllMocks();
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	describe('hydrate', () => {
		it('loads defaults when localStorage is empty', () => {
			preferences.hydrate();
			expect(preferences.current).toEqual(DEFAULT_PREFERENCES);
		});

		it('loads persisted overrides from localStorage', () => {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(withOverrides({ text_size: 'large' })));
			preferences.hydrate();
			expect(preferences.current.text_size).toBe('large');
		});

		it('falls back to defaults on corrupt localStorage', () => {
			localStorage.setItem(STORAGE_KEY, '{not valid json');
			preferences.hydrate();
			expect(preferences.current).toEqual(DEFAULT_PREFERENCES);
		});
	});

	describe('reconcile', () => {
		it('anonymous (null server) keeps localStorage authoritative and does not sync', async () => {
			const fetchSpy = vi.spyOn(globalThis, 'fetch');
			localStorage.setItem(STORAGE_KEY, JSON.stringify(withOverrides({ text_size: 'small' })));
			await preferences.reconcile(null);
			expect(preferences.current.text_size).toBe('small');
			expect(preferences.synced).toBe(true);
			expect(fetchSpy).not.toHaveBeenCalled();
		});

		it('server wins when it has overrides', async () => {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(withOverrides({ text_size: 'small' })));
			await preferences.reconcile(withOverrides({ text_size: 'large' }));
			expect(preferences.current.text_size).toBe('large');
			expect(JSON.parse(localStorage.getItem(STORAGE_KEY)!).text_size).toBe('large');
		});

		it('pushes local up on first login when server is all-default', async () => {
			const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
				new Response(JSON.stringify({ data: withOverrides({ high_contrast: 'on' }) }), {
					status: 200,
					headers: { 'content-type': 'application/json' }
				})
			);
			localStorage.setItem(STORAGE_KEY, JSON.stringify(withOverrides({ high_contrast: 'on' })));

			await preferences.reconcile({ ...DEFAULT_PREFERENCES });

			expect(fetchSpy).toHaveBeenCalledWith(
				'/preferences',
				expect.objectContaining({ method: 'PATCH' })
			);
			// Only the overriding key is pushed.
			const body = JSON.parse((fetchSpy.mock.calls[0][1] as RequestInit).body as string);
			expect(body).toEqual({ high_contrast: 'on' });
			expect(preferences.current.high_contrast).toBe('on');
		});

		it('does not push when both server and local are all-default', async () => {
			const fetchSpy = vi.spyOn(globalThis, 'fetch');
			await preferences.reconcile({ ...DEFAULT_PREFERENCES });
			expect(fetchSpy).not.toHaveBeenCalled();
			expect(preferences.current).toEqual(DEFAULT_PREFERENCES);
		});
	});

	describe('preview vs commit', () => {
		it('preview updates state but does NOT persist to localStorage', () => {
			preferences.preview({ text_size: 'large' });
			expect(preferences.current.text_size).toBe('large');
			expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
		});

		it('revertToPersisted discards an unpersisted preview', () => {
			preferences.preview({ text_size: 'large' });
			preferences.revertToPersisted();
			expect(preferences.current.text_size).toBe('auto');
		});

		it('commit persists to localStorage and PATCHes the server', async () => {
			const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
				new Response(JSON.stringify({ data: withOverrides({ text_size: 'large' }) }), {
					status: 200,
					headers: { 'content-type': 'application/json' }
				})
			);
			await preferences.commit({ text_size: 'large' });
			expect(JSON.parse(localStorage.getItem(STORAGE_KEY)!).text_size).toBe('large');
			expect(fetchSpy).toHaveBeenCalledWith(
				'/preferences',
				expect.objectContaining({ method: 'PATCH' })
			);
		});

		it('commit still persists locally when the server sync fails', async () => {
			vi.spyOn(globalThis, 'fetch').mockResolvedValue(new Response(null, { status: 500 }));
			await preferences.commit({ text_size: 'small' });
			expect(preferences.current.text_size).toBe('small');
			expect(JSON.parse(localStorage.getItem(STORAGE_KEY)!).text_size).toBe('small');
		});
	});

	describe('persisted', () => {
		it('reflects localStorage, not a live preview', () => {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(withOverrides({ text_size: 'large' })));
			preferences.hydrate();
			// A preview mutates `current` but must NOT change `persisted`.
			preferences.preview({ text_size: 'small' });
			expect(preferences.current.text_size).toBe('small');
			expect(preferences.persisted.text_size).toBe('large');
		});

		it('is defaults when nothing is stored', () => {
			expect(preferences.persisted).toEqual(DEFAULT_PREFERENCES);
		});
	});

	describe('resetToDefaults', () => {
		it('sets all preferences to defaults, applies, and persists', async () => {
			vi.spyOn(globalThis, 'fetch').mockResolvedValue(
				new Response(JSON.stringify({ data: { ...DEFAULT_PREFERENCES } }), {
					status: 200,
					headers: { 'content-type': 'application/json' }
				})
			);
			localStorage.setItem(
				STORAGE_KEY,
				JSON.stringify(withOverrides({ text_size: 'large', reduce_motion: 'on' }))
			);
			preferences.hydrate();

			await preferences.resetToDefaults();

			expect(preferences.current).toEqual(DEFAULT_PREFERENCES);
			expect(JSON.parse(localStorage.getItem(STORAGE_KEY)!)).toEqual(DEFAULT_PREFERENCES);
		});

		it('syncs the full default set so prior server overrides are overwritten', async () => {
			const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
				new Response(JSON.stringify({ data: { ...DEFAULT_PREFERENCES } }), {
					status: 200,
					headers: { 'content-type': 'application/json' }
				})
			);
			await preferences.resetToDefaults();
			const body = JSON.parse((fetchSpy.mock.calls[0][1] as RequestInit).body as string);
			expect(body).toEqual(DEFAULT_PREFERENCES);
		});
	});
});
