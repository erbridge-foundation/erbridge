import { sveltekit } from '@sveltejs/kit/vite';
import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { defineConfig } from 'vitest/config';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// Version + commit are git-tag-derived and computed outside the image (see
// RELEASING.md), passed in as the APP_VERSION / GIT_COMMIT_SHA build-time env vars
// by CI and `just docker-build-frontend`. They are inlined into the client bundle
// as import.meta.env.PUBLIC_UI_VERSION / PUBLIC_GIT_COMMIT (consumed by /about).
// With no env (a plain local build) UI version falls back to the frozen,
// non-authoritative package.json sentinel and commit to "unknown".
const pkg = JSON.parse(
	readFileSync(fileURLToPath(new URL('./package.json', import.meta.url)), 'utf-8')
) as { version: string };

const uiVersion = process.env.APP_VERSION?.trim() || pkg.version;
const gitCommit = process.env.GIT_COMMIT_SHA?.trim() || 'unknown';

export default defineConfig({
	plugins: [
		sveltekit(),
		// Compile-time i18n. Messages compile to tree-shakeable functions in
		// src/lib/paraglide/messages; the runtime to src/lib/paraglide/runtime.
		// Locale resolution: the user's cookie (written by the preferences store),
		// then the browser Accept-Language header, then the base locale (en). No
		// `url` strategy — E-R Bridge is an authenticated tool, so no /en/ path
		// prefixes (see the i18n change's design.md).
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
			strategy: ['cookie', 'preferredLanguage', 'baseLocale']
		})
	],
	define: {
		'import.meta.env.PUBLIC_UI_VERSION': JSON.stringify(uiVersion),
		'import.meta.env.PUBLIC_GIT_COMMIT': JSON.stringify(gitCommit)
	},
	test: {
		environment: 'jsdom',
		setupFiles: ['./vitest.setup.ts'],
		include: ['src/**/*.{test,spec}.{ts,svelte.ts}'],
		globals: false,
		// Resolve Svelte to its browser build so components can mount in jsdom.
		// Without this, vitest picks up the server (SSR) build and mount(...)
		// throws lifecycle_function_unavailable.
		server: {
			deps: {
				inline: ['@testing-library/svelte']
			}
		}
	},
	resolve: {
		conditions: ['browser']
	}
});
