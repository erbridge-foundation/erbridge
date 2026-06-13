/**
 * Regression guard for the forced-colors / native-control accessibility rules
 * in app.css (see openspec accessibility-preferences).
 *
 * jsdom cannot emulate `forced-colors: active` or `color-scheme`, so these
 * outcomes are exercised for real in a Playwright e2e spec
 * (tests/e2e/forced-colors.spec.ts). This static check only guards against the
 * declarations being accidentally removed.
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, it, expect } from 'vitest';

// Vitest runs with cwd at the frontend/ project root.
const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');

describe('app.css accessibility rendering hints', () => {
	it('declares color-scheme: dark (native controls render dark)', () => {
		expect(css).toMatch(/color-scheme:\s*dark/);
	});

	it('declares accent-color: var(--sky) (native form controls themed)', () => {
		expect(css).toMatch(/accent-color:\s*var\(--sky\)/);
	});

	it('includes a forced-colors: active block', () => {
		expect(css).toMatch(/@media\s*\(forced-colors:\s*active\)/);
	});
});
