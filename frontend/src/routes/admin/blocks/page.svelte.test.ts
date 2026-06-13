import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup, within } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';
import BlocksPage from './+page.svelte';
import type {
	MeResponse,
	CharacterSearchResultDto,
	EsiCharacterSearchResultDto
} from '$lib/api';

const BlocksPageComponent = BlocksPage;

// The logged-in admin. account.id drives local self-matching; characters[] drives
// the ESI char-level fallback.
const me: MeResponse = {
	account: { id: 'acc-1', status: 'active', is_server_admin: true, created_at: '2024-01-01T00:00:00Z' },
	characters: [
		{
			id: 'c-1',
			eve_character_id: 1001,
			name: 'AdminMain',
			corporation_id: 1,
			corporation_name: 'Corp',
			alliance_id: null,
			alliance_name: null,
			is_main: true,
			portrait_url: '/p/1001',
			token_status: 'active'
		}
	]
};

function localResult(over: Partial<CharacterSearchResultDto> = {}): CharacterSearchResultDto {
	return {
		eve_character_id: 2001,
		name: 'SomeoneElse',
		is_main: true,
		account_id: 'acc-2',
		portrait_url: '/p/2001',
		already_blocked: false,
		...over
	};
}

function esiResult(over: Partial<EsiCharacterSearchResultDto> = {}): EsiCharacterSearchResultDto {
	return {
		eve_character_id: 3001,
		name: 'EsiStranger',
		portrait_url: '/p/3001',
		already_blocked: false,
		...over
	};
}

// data = layout data (me) merged with the page's own (blocks). form carries the
// search results the picker renders.
function props(
	results: (CharacterSearchResultDto | EsiCharacterSearchResultDto)[],
	action: 'search' | 'esiSearch'
): ComponentProps<typeof BlocksPageComponent> {
	return {
		data: { me, blocks: [] },
		form: { action, query: 'who', results }
	} as unknown as ComponentProps<typeof BlocksPageComponent>;
}

function resultRow(name: string): HTMLElement {
	return screen.getByText(name).closest('li') as HTMLElement;
}

afterEach(() => cleanup());

describe('/admin/blocks self-block affordance', () => {
	it('marks a local result on the admin\'s own account as non-selectable', () => {
		render(BlocksPageComponent, {
			props: props([localResult({ name: 'MyAlt', account_id: 'acc-1' })], 'search')
		});
		const row = resultRow('MyAlt');
		expect(within(row).getByText('You')).toBeInTheDocument();
		expect(within(row).queryByRole('button')).not.toBeInTheDocument();
	});

	it('marks an ESI result matching one of the admin\'s own characters as non-selectable', () => {
		render(BlocksPageComponent, {
			props: props([esiResult({ name: 'AdminMain', eve_character_id: 1001 })], 'esiSearch')
		});
		const row = resultRow('AdminMain');
		expect(within(row).getByText('You')).toBeInTheDocument();
		expect(within(row).queryByRole('button')).not.toBeInTheDocument();
	});

	it('renders the normal Select control for a result on another account', () => {
		render(BlocksPageComponent, {
			props: props([localResult({ name: 'SomeoneElse', account_id: 'acc-2' })], 'search')
		});
		const row = resultRow('SomeoneElse');
		expect(within(row).queryByText('You')).not.toBeInTheDocument();
		expect(within(row).getByRole('button')).toBeInTheDocument();
	});
});
