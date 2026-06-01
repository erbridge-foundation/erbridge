import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';

// Scope a text query to within a container element (the dialog), so the same
// name appearing in the search-results list does not collide.
const getByTextScoped = (container: HTMLElement, text: string) =>
	within(container).getByText(text);
const queryByTextScoped = (container: HTMLElement, text: string) =>
	within(container).queryByText(text);

vi.mock('$app/forms', () => ({
	enhance: () => ({ destroy: () => {} })
}));

const CharactersPage = (await import('./+page.svelte')).default;

const account = {
	id: 'a1',
	status: 'active',
	is_server_admin: false,
	created_at: 'now',
	characters: [
		{ eve_character_id: 1, name: 'MainPilot', is_main: true, token_status: 'active' as const },
		{ eve_character_id: 2, name: 'SoldAlt', is_main: false, token_status: 'owner_mismatch' as const },
		{ eve_character_id: 3, name: 'StaleAlt', is_main: false, token_status: 'expired' as const }
	]
};

const searchForm = {
	action: 'search' as const,
	query: 'pilot',
	results: [
		{
			eve_character_id: 1,
			name: 'MainPilot',
			is_main: true,
			account_id: 'a1',
			portrait_url: '',
			already_blocked: false
		},
		{
			eve_character_id: 99,
			name: 'OrphanGuy',
			is_main: false,
			account_id: null,
			portrait_url: '',
			already_blocked: false
		}
	]
};

function props(form: unknown = null): ComponentProps<typeof CharactersPage> {
	return {
		data: { accounts: [account] },
		form
	} as unknown as ComponentProps<typeof CharactersPage>;
}

afterEach(() => cleanup());

describe('admin/characters page', () => {
	it('lists search results with an inspect action for owned characters', () => {
		render(CharactersPage, { props: props(searchForm) });
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		// Orphan result shows the no-account label, not an inspect button.
		expect(screen.getByText('OrphanGuy')).toBeInTheDocument();
		expect(screen.getAllByRole('button', { name: /inspect account/i })).toHaveLength(1);
	});

	it('opens the inspect dialog showing all characters with token states', async () => {
		render(CharactersPage, { props: props(searchForm) });
		await fireEvent.click(screen.getByRole('button', { name: /inspect account/i }));

		const dialog = screen.getByRole('dialog');
		expect(dialog).toBeInTheDocument();
		const within = (text: string) => getByTextScoped(dialog, text);
		// All three characters of the account are listed in the dialog.
		expect(within('SoldAlt')).toBeInTheDocument();
		expect(within('StaleAlt')).toBeInTheDocument();
		// The transferred character's status surfaces.
		expect(within('transferred')).toBeInTheDocument();
	});

	it('filters the dialog to only characters needing attention', async () => {
		render(CharactersPage, { props: props(searchForm) });
		await fireEvent.click(screen.getByRole('button', { name: /inspect account/i }));

		const dialog = screen.getByRole('dialog');
		// The main is shown in the dialog initially.
		expect(getByTextScoped(dialog, 'MainPilot')).toBeInTheDocument();

		await fireEvent.click(screen.getByRole('button', { name: /needs attention/i }));

		// The active main is filtered out of the dialog; the two problem
		// characters remain.
		expect(queryByTextScoped(dialog, 'MainPilot')).toBeNull();
		expect(getByTextScoped(dialog, 'SoldAlt')).toBeInTheDocument();
		expect(getByTextScoped(dialog, 'StaleAlt')).toBeInTheDocument();
	});
});
