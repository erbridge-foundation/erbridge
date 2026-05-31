import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: 'tests/e2e',
	timeout: 30_000,
	retries: 0,
	globalSetup: './tests/e2e/mock-backend.ts',
	use: {
		baseURL: 'http://localhost:4173'
	},
	webServer: {
		command:
			'pnpm run build && BACKEND_INTERNAL_URL=http://127.0.0.1:9100 ESI_PUBLIC_BASE=http://127.0.0.1:9100/esi ORIGIN=http://localhost:4173 PORT=4173 node build',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 120_000
	}
});
