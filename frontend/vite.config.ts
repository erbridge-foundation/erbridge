import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
	plugins: [sveltekit()],
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
