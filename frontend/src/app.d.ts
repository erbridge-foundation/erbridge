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

	// UI version inlined from package.json at build time (see vite.config.ts).
	interface ImportMetaEnv {
		readonly PUBLIC_UI_VERSION: string;
	}
	interface ImportMeta {
		readonly env: ImportMetaEnv;
	}
}

export {};
