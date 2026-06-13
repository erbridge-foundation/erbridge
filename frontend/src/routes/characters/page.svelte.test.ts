// Behavioural coverage of the add-character bound-elsewhere conflict notice:
// it renders only when the URL carries `?add_conflict=bound_elsewhere`, it is
// dismissible, and the flag is stripped from the URL via replaceState so a
// reload does not re-show it. The rest of the page is a straightforward render
// of `data.characters` and is exercised by the e2e suite.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';

// Mutable `page` stub — each test sets `pageStub.url` before rendering.
const pageStub: { url: URL; state: Record<string, unknown> } = {
	url: new URL('http://localhost/characters'),
	state: {}
};
vi.mock('$app/state', () => ({
	get page() {
		return pageStub;
	}
}));

const replaceState = vi.fn();
vi.mock('$app/navigation', () => ({
	replaceState: (...args: unknown[]) => replaceState(...args)
}));

const CharactersPage = (await import('./+page.svelte')).default;

const character = {
	id: 'c1',
	name: 'Main Pilot',
	is_main: true,
	token_status: 'active' as const,
	portrait_url: 'https://images.evetech.net/c1',
	corporation_name: 'Test Corp',
	alliance_name: null
};

function props(url: string): ComponentProps<typeof CharactersPage> {
	pageStub.url = new URL(url);
	return { data: { characters: [character] }, form: null } as unknown as ComponentProps<
		typeof CharactersPage
	>;
}

describe('characters page — bound-elsewhere conflict notice', () => {
	beforeEach(() => {
		replaceState.mockClear();
		pageStub.state = {};
	});

	afterEach(() => {
		cleanup();
	});

	it('renders the notice when add_conflict=bound_elsewhere', () => {
		render(CharactersPage, {
			props: props('http://localhost/characters?add_conflict=bound_elsewhere')
		});
		const alert = screen.getByRole('alert');
		expect(alert).toHaveTextContent(/already linked to another account/i);
	});

	it('does not render the notice without the flag', () => {
		render(CharactersPage, { props: props('http://localhost/characters') });
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('does not render the notice for an unrelated add_conflict value', () => {
		render(CharactersPage, {
			props: props('http://localhost/characters?add_conflict=something_else')
		});
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('strips the add_conflict flag from the URL via replaceState', () => {
		render(CharactersPage, {
			props: props('http://localhost/characters?add_conflict=bound_elsewhere')
		});
		expect(replaceState).toHaveBeenCalledTimes(1);
		const [url] = replaceState.mock.calls[0];
		expect((url as URL).searchParams.has('add_conflict')).toBe(false);
	});

	it('does not call replaceState when there is no flag to strip', () => {
		render(CharactersPage, { props: props('http://localhost/characters') });
		expect(replaceState).not.toHaveBeenCalled();
	});

	it('dismisses the notice when the close button is activated', async () => {
		render(CharactersPage, {
			props: props('http://localhost/characters?add_conflict=bound_elsewhere')
		});
		expect(screen.queryByRole('alert')).not.toBeNull();
		await fireEvent.click(screen.getByRole('button', { name: /dismiss notice/i }));
		expect(screen.queryByRole('alert')).toBeNull();
	});
});
