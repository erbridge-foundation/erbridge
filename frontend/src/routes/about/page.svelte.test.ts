import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/svelte';
import AboutPage from './+page.svelte';
import type { HealthResponse } from '$lib/api';
import type { PageData } from './$types';

const healthy: HealthResponse = {
	status: 'ok',
	version: '0.2.5',
	commit: 'deadbee',
	components: [{ name: 'db', status: 'ok' }]
};

// /about page data is the layout data (me/meError) merged with the page's own
// (health/healthError). Build the full shape so props type-check.
function pageData(
	health: HealthResponse | null,
	healthError: { message: string } | null
): PageData {
	return { me: null, meError: null, health, healthError } as PageData;
}

afterEach(() => cleanup());

describe('/about +page.svelte', () => {
	it('renders the API version and commit when health is reachable', () => {
		render(AboutPage, { props: { data: pageData(healthy, null) } });
		expect(screen.getByText(/0\.2\.5/)).toBeInTheDocument();
		expect(screen.getByText(/deadbee/)).toBeInTheDocument();
		expect(screen.queryByText('API: unreachable')).not.toBeInTheDocument();
	});

	it('renders "API: unreachable" when health is null', () => {
		render(AboutPage, { props: { data: pageData(null, { message: 'down' }) } });
		expect(screen.getByText('API: unreachable')).toBeInTheDocument();
	});

	it('always renders the CCP disclaimer (guard against accidental deletion)', () => {
		render(AboutPage, { props: { data: pageData(null, null) } });
		// The disclaimer must contain the literal "CCP hf." substring.
		expect(document.body.textContent).toContain('CCP hf.');
	});

	it('links to the GitHub repo in a new tab', () => {
		render(AboutPage, { props: { data: pageData(healthy, null) } });
		const link = screen.getByRole('link', { name: /Source on GitHub/ });
		expect(link).toHaveAttribute('href', 'https://github.com/erbridge-foundation/erbridge');
		expect(link).toHaveAttribute('target', '_blank');
		expect(link).toHaveAttribute('rel', 'noopener noreferrer');
	});

	it('renders all acknowledgement links opening in new tabs', () => {
		render(AboutPage, { props: { data: pageData(healthy, null) } });
		for (const href of [
			'https://tripwiremap.app/',
			'https://wanderer.ltd/',
			'https://anoikis.info/',
			'https://www.eve-scout.com/'
		]) {
			const link = document.querySelector(`a[href="${href}"]`);
			expect(link).not.toBeNull();
			expect(link).toHaveAttribute('target', '_blank');
			expect(link).toHaveAttribute('rel', 'noopener noreferrer');
		}
	});
});
