import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, screen, cleanup } from '@testing-library/svelte';
import { writable } from 'svelte/store';
import { createRawSnippet } from 'svelte';
import type { LayoutData } from './$types';

// Minimal page content so the layout's `{@render children()}` has a snippet.
const children = createRawSnippet(() => ({
	render: () => '<div data-testid="page-content"></div>'
}));

// `+layout.svelte` reads `$page.url.pathname` from `$app/stores`. Drive it with a
// writable so each test can set the pathname before rendering, without tearing
// down the module graph (resetting modules duplicates the Svelte runtime and
// breaks $effect mount context).
const pageStore = writable({ url: new URL('http://localhost/') });
vi.mock('$app/stores', () => ({ page: pageStore }));

// The preference store hydrate/reconcile run in an $effect on mount; stub them
// so the layout test doesn't touch localStorage or the network.
vi.mock('$lib/preferences/store.svelte', () => ({
	preferences: { hydrate: vi.fn(), reconcile: vi.fn().mockResolvedValue(undefined) }
}));

const Layout = (await import('./+layout.svelte')).default;

function layoutData(overrides: Partial<LayoutData> = {}): LayoutData {
	return { me: null, meError: null, serverPrefs: null, ...overrides } as LayoutData;
}

afterEach(() => cleanup());

function renderAt(pathname: string, data: LayoutData = layoutData()) {
	pageStore.set({ url: new URL(`http://localhost${pathname}`) });
	return render(Layout, { props: { data, children } });
}

describe('+layout.svelte chrome-less routes', () => {
	it('renders the global nav on a normal authenticated route', () => {
		const { container } = renderAt(
			'/',
			layoutData({
				me: {
					account: { id: 'a', status: 'active', is_server_admin: false, created_at: 'now' },
					characters: []
				} as LayoutData['me']
			})
		);
		expect(container.querySelector('header.global-nav')).not.toBeNull();
	});

	it('hides the global nav on /login (chrome-less) and applies .chromeless', () => {
		const { container } = renderAt('/login');
		expect(container.querySelector('header.global-nav')).toBeNull();
		expect(container.querySelector('.app.chromeless')).not.toBeNull();
	});

	it('hides the global nav on /blocked (chrome-less) and applies .chromeless', () => {
		const { container } = renderAt('/blocked');
		expect(container.querySelector('header.global-nav')).toBeNull();
		expect(container.querySelector('.app.chromeless')).not.toBeNull();
	});

	it('does NOT render the meError banner on a chrome-less route', () => {
		renderAt('/blocked', layoutData({ meError: 'upstream failed' }));
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('renders the meError banner on a normal route', () => {
		renderAt('/', layoutData({ meError: 'upstream failed' }));
		expect(screen.getByRole('alert')).toBeInTheDocument();
	});
});
