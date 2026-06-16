import { describe, it, expect, beforeEach } from 'vitest';
import { load, loadTab, save, clearTab } from './placement';
import type { Positions } from './types';

const MAP = 'map-1';
const a: Positions = { A: { x: 1, y: 2 } };
const b: Positions = { B: { x: 3, y: 4 } };

describe('placement (localStorage seam)', () => {
	beforeEach(() => localStorage.clear());

	it('round-trips a tab synchronously with debounce 0', () => {
		save(MAP, 'home', a, 0);
		expect(loadTab(MAP, 'home')).toEqual(a);
	});

	it('keeps tabs separate within a map', () => {
		save(MAP, 'home', a, 0);
		save(MAP, 'deep', b, 0);
		expect(loadTab(MAP, 'home')).toEqual(a);
		expect(loadTab(MAP, 'deep')).toEqual(b);
		expect(load(MAP)).toEqual({ home: a, deep: b });
	});

	it('returns an empty map for an unknown map / tab', () => {
		expect(load('nope')).toEqual({});
		expect(loadTab(MAP, 'missing')).toEqual({});
	});

	it('clearTab forgets only that tab (the redo-layout fallback to seed)', () => {
		save(MAP, 'home', a, 0);
		save(MAP, 'deep', b, 0);
		clearTab(MAP, 'home');
		expect(loadTab(MAP, 'home')).toEqual({});
		expect(loadTab(MAP, 'deep')).toEqual(b); // untouched
	});

	it('survives a corrupt blob without throwing', () => {
		localStorage.setItem('erbridge:map-placement:map-1', '{not json');
		expect(load(MAP)).toEqual({});
	});

	it('debounced save eventually persists', async () => {
		save(MAP, 'home', a, 10);
		expect(loadTab(MAP, 'home')).toEqual({}); // not yet
		await new Promise((r) => setTimeout(r, 25));
		expect(loadTab(MAP, 'home')).toEqual(a);
	});
});
