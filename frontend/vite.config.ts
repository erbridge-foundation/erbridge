import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// UI version, sourced from package.json at build time and inlined into the
// client bundle as import.meta.env.PUBLIC_UI_VERSION (consumed by /about).
const pkg = JSON.parse(
	readFileSync(fileURLToPath(new URL('./package.json', import.meta.url)), 'utf-8')
) as { version: string };

export default defineConfig({
	plugins: [sveltekit()],
	define: {
		'import.meta.env.PUBLIC_UI_VERSION': JSON.stringify(pkg.version)
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
