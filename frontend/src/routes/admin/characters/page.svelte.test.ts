import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';

const CharactersPage = (await import('./+page.svelte')).default;

type Char = {
	eve_character_id: number;
	name: string;
	is_main: boolean;
	token_status: 'active' | 'expired' | 'owner_mismatch';
};
type Account = {
	id: string;
	status: string;
	is_server_admin: boolean;
	created_at: string;
	characters: Char[];
};

// Account whose main has two problem alts (one expired, one transferred).
const flagged: Account = {
	id: 'a1',
	status: 'active',
	is_server_admin: false,
	created_at: '2023-01-01T00:00:00Z',
	characters: [
		{ eve_character_id: 1, name: 'MainPilot', is_main: true, token_status: 'active' },
		{ eve_character_id: 2, name: 'SoldAlt', is_main: false, token_status: 'owner_mismatch' },
		{ eve_character_id: 3, name: 'StaleAlt', is_main: false, token_status: 'expired' }
	]
};

// Clean single-character account; also a server admin.
const clean: Account = {
	id: 'a2',
	status: 'active',
	is_server_admin: true,
	created_at: '2024-06-01T00:00:00Z',
	characters: [{ eve_character_id: 10, name: 'CleanCaptain', is_main: true, token_status: 'active' }]
};

// Account with NO main flagged — label must fall back to first character by name.
const noMain: Account = {
	id: 'a3',
	status: 'disabled',
	is_server_admin: false,
	created_at: '2022-03-03T00:00:00Z',
	characters: [
		{ eve_character_id: 20, name: 'Zeta', is_main: false, token_status: 'active' },
		{ eve_character_id: 21, name: 'Alpha', is_main: false, token_status: 'active' }
	]
};

function props(accounts: Account[]): ComponentProps<typeof CharactersPage> {
	return { data: { accounts } } as unknown as ComponentProps<typeof CharactersPage>;
}

function accountRow(label: string): HTMLElement {
	return screen.getByText(label).closest('tr') as HTMLElement;
}

afterEach(() => cleanup());

describe('admin/characters grid', () => {
	it('renders one row per account labelled by its main character', () => {
		render(CharactersPage, { props: props([flagged, clean]) });
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		expect(screen.getByText('CleanCaptain')).toBeInTheDocument();
	});

	it('falls back to the first character by name when no main is flagged', () => {
		render(CharactersPage, { props: props([noMain]) });
		// 'Alpha' sorts before 'Zeta', so it is the label.
		expect(screen.getByText('Alpha')).toBeInTheDocument();
		const row = accountRow('Alpha');
		// 'Zeta' is not promoted to the account label cell (it appears only on expand).
		expect(within(row).queryByText('Zeta')).toBeNull();
	});

	it('shows the issues roll-up while the row is collapsed', () => {
		render(CharactersPage, { props: props([flagged]) });
		const row = accountRow('MainPilot');
		expect(within(row).getByText('1 expired')).toBeInTheDocument();
		expect(within(row).getByText('1 transferred')).toBeInTheDocument();
	});

	it('renders an empty-state when there are no accounts', () => {
		render(CharactersPage, { props: props([]) });
		expect(screen.getByText('No accounts.')).toBeInTheDocument();
	});

	it('expands and collapses the per-character token table', async () => {
		render(CharactersPage, { props: props([flagged]) });
		// Collapsed: alts are not rendered.
		expect(screen.queryByText('SoldAlt')).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: /show characters for MainPilot/i }));
		expect(screen.getByText('SoldAlt')).toBeInTheDocument();
		expect(screen.getByText('StaleAlt')).toBeInTheDocument();
		// The transferred alt's token status appears in the expanded detail table.
		const detail = screen.getByText('SoldAlt').closest('table') as HTMLElement;
		expect(within(detail).getByText('transferred')).toBeInTheDocument();

		await fireEvent.click(screen.getByRole('button', { name: /hide characters for MainPilot/i }));
		expect(screen.queryByText('SoldAlt')).toBeNull();
	});

	it('filters by character name, matching alt names too', async () => {
		render(CharactersPage, { props: props([flagged, clean]) });
		const input = screen.getByLabelText(/filter accounts by character name/i);

		// Filtering by an alt name surfaces that alt's account row.
		await fireEvent.input(input, { target: { value: 'soldalt' } });
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		expect(screen.queryByText('CleanCaptain')).toBeNull();
	});

	it('clears the text filter via the clear button', async () => {
		render(CharactersPage, { props: props([flagged, clean]) });
		const input = screen.getByLabelText(/filter accounts by character name/i);

		// No clear button until there is text.
		expect(screen.queryByRole('button', { name: /clear filter/i })).toBeNull();

		await fireEvent.input(input, { target: { value: 'soldalt' } });
		expect(screen.queryByText('CleanCaptain')).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: /clear filter/i }));
		// Filter is emptied: both accounts show again and the button disappears.
		expect((input as HTMLInputElement).value).toBe('');
		expect(screen.getByText('CleanCaptain')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /clear filter/i })).toBeNull();
	});

	it('shows a no-match message when the filter excludes everything', async () => {
		render(CharactersPage, { props: props([flagged]) });
		const input = screen.getByLabelText(/filter accounts by character name/i);
		await fireEvent.input(input, { target: { value: 'nobody' } });
		expect(screen.getByText(/no accounts match/i)).toBeInTheDocument();
	});

	it('filters at account level via status chips', async () => {
		render(CharactersPage, { props: props([flagged, clean]) });
		// 'expired' chip keeps only the account with an expired character.
		await fireEvent.click(screen.getByRole('button', { name: /^expired$/i }));
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		expect(screen.queryByText('CleanCaptain')).toBeNull();

		// 'transferred' chip keeps the flagged account (has an owner_mismatch char).
		await fireEvent.click(screen.getByRole('button', { name: /^transferred$/i }));
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		expect(screen.queryByText('CleanCaptain')).toBeNull();

		// Back to 'all' shows both.
		await fireEvent.click(screen.getByRole('button', { name: /^all$/i }));
		expect(screen.getByText('MainPilot')).toBeInTheDocument();
		expect(screen.getByText('CleanCaptain')).toBeInTheDocument();
	});

	it('toggles account-name sort order', async () => {
		render(CharactersPage, { props: props([flagged, clean]) });
		const header = screen.getByRole('button', { name: /^Account$/i });

		// Ascending: CleanCaptain before MainPilot.
		await fireEvent.click(header);
		let labels = screen
			.getAllByRole('row')
			.map((r) => r.querySelector('.account-cell')?.textContent?.trim())
			.filter(Boolean);
		expect(labels).toEqual(['CleanCaptain', 'MainPilot']);

		// Descending: order reverses.
		await fireEvent.click(header);
		labels = screen
			.getAllByRole('row')
			.map((r) => r.querySelector('.account-cell')?.textContent?.trim())
			.filter(Boolean);
		expect(labels).toEqual(['MainPilot', 'CleanCaptain']);
	});

	it('sorts by issue severity (transferred outranks clean)', async () => {
		render(CharactersPage, { props: props([clean, flagged]) });
		const header = screen.getByRole('button', { name: /^Issues$/i });

		// Descending (worst first): the flagged account leads.
		await fireEvent.click(header); // asc
		await fireEvent.click(header); // desc
		const labels = screen
			.getAllByRole('row')
			.map((r) => r.querySelector('.account-cell')?.textContent?.trim())
			.filter(Boolean);
		expect(labels[0]).toBe('MainPilot');
	});
});
