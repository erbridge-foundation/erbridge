import type { MeResponse } from '$lib/api';

// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		interface Locals {
			me: MeResponse | null;
		}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	// UI version + git commit inlined at build time (see vite.config.ts):
	// git-tag-derived APP_VERSION / GIT_COMMIT_SHA, with documented fallbacks.
	interface ImportMetaEnv {
		readonly PUBLIC_UI_VERSION: string;
		readonly PUBLIC_GIT_COMMIT: string;
	}
	interface ImportMeta {
		readonly env: ImportMetaEnv;
	}
}

export {};
