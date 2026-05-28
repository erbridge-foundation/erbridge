import adapter from '@sveltejs/adapter-node';

// Same APP_VERSION the vite config consumes for PUBLIC_UI_VERSION. The frontend
// Dockerfile promotes APP_VERSION to ENV before `pnpm run build`, so it is
// present here at build time. With no env (a plain local build), fall back to a
// fixed, stable string so $app/state's `updated` does not flip on every local
// rebuild and spam the reload banner in dev.
const appVersion = process.env.APP_VERSION?.trim() || 'dev';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => (filename.split(/[/\\]/).includes('node_modules') ? undefined : true)
	},
	kit: {
		adapter: adapter({
			out: 'build',
			precompress: true
		}),
		version: {
			name: appVersion,
			pollInterval: 60000
		}
	}
};

export default config;
