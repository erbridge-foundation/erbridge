import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup, within } from '@testing-library/svelte';
import MemberPicker from './MemberPicker.svelte';

afterEach(() => cleanup());

const aCharacter = { id: 'char-uuid-1', eve_character_id: 7, name: 'Wasp 223' };
const aCorp = { eve_entity_id: 98000001, name: 'Test Corp' };
const anAlliance = { eve_entity_id: 99000001, name: 'Test Alliance' };

function row(container: HTMLElement, name: string): HTMLElement {
	const li = within(container)
		.getByText(name)
		.closest('li') as HTMLElement | null;
	if (!li) throw new Error(`no result row for ${name}`);
	return li;
}

describe('MemberPicker inline add form — identifier by type', () => {
	it('a character row carries member_type, character_id, eve_entity_id + name', () => {
		const { container } = render(MemberPicker, {
			props: { characters: [aCharacter], searched: true }
		});
		const li = row(container, 'Wasp 223');
		const form = li.querySelector('form.add-inline') as HTMLFormElement;
		expect(form.action).toContain('?/addMember');
		expect((form.querySelector('[name=member_type]') as HTMLInputElement).value).toBe('character');
		expect((form.querySelector('[name=character_id]') as HTMLInputElement).value).toBe('char-uuid-1');
		expect((form.querySelector('[name=name]') as HTMLInputElement).value).toBe('Wasp 223');
		// A character now also carries its durable EVE id (the eve_character_id),
		// so the audit snapshot is uniform with corp/alliance members.
		expect((form.querySelector('[name=eve_entity_id]') as HTMLInputElement).value).toBe('7');
		// Submit button is the inline "add", not a select/select-then-scroll.
		expect(within(li).getByRole('button', { name: 'add member' })).toBeInTheDocument();
	});

	it('a corporation row carries eve_entity_id and no character_id', () => {
		const { container } = render(MemberPicker, {
			props: { corporations: [aCorp], searched: true }
		});
		const form = row(container, 'Test Corp').querySelector('form.add-inline') as HTMLFormElement;
		expect((form.querySelector('[name=member_type]') as HTMLInputElement).value).toBe('corporation');
		expect((form.querySelector('[name=eve_entity_id]') as HTMLInputElement).value).toBe('98000001');
		expect(form.querySelector('[name=character_id]')).toBeNull();
	});

	it('an alliance row carries eve_entity_id', () => {
		const { container } = render(MemberPicker, {
			props: { alliances: [anAlliance], searched: true }
		});
		const form = row(container, 'Test Alliance').querySelector('form.add-inline') as HTMLFormElement;
		expect((form.querySelector('[name=member_type]') as HTMLInputElement).value).toBe('alliance');
		expect((form.querySelector('[name=eve_entity_id]') as HTMLInputElement).value).toBe('99000001');
	});
});

describe('MemberPicker inline permission gating', () => {
	it('offers manage/admin for a character row', () => {
		const { container } = render(MemberPicker, {
			props: { characters: [aCharacter], searched: true }
		});
		const select = row(container, 'Wasp 223').querySelector('select') as HTMLSelectElement;
		const values = Array.from(select.options).map((o) => o.value);
		expect(values).toEqual(['read', 'read_write', 'manage', 'admin', 'deny']);
	});

	it('withholds manage/admin for a corporation row', () => {
		const { container } = render(MemberPicker, {
			props: { corporations: [aCorp], searched: true }
		});
		const select = row(container, 'Test Corp').querySelector('select') as HTMLSelectElement;
		const values = Array.from(select.options).map((o) => o.value);
		expect(values).toEqual(['read', 'read_write', 'deny']);
	});
});

describe('MemberPicker portraits', () => {
	it('renders a character portrait from the EVE image CDN keyed by eve_character_id', () => {
		const { container } = render(MemberPicker, {
			props: { characters: [aCharacter], searched: true }
		});
		const img = container.querySelector('img.portrait') as HTMLImageElement;
		expect(img).not.toBeNull();
		expect(img.src).toContain('/characters/7/portrait');
	});

	it('renders a corporation logo keyed by eve_entity_id', () => {
		const { container } = render(MemberPicker, {
			props: { corporations: [aCorp], searched: true }
		});
		const img = container.querySelector('img.portrait') as HTMLImageElement;
		expect(img.src).toContain('/corporations/98000001/logo');
	});

	it('renders an alliance logo keyed by eve_entity_id', () => {
		const { container } = render(MemberPicker, {
			props: { alliances: [anAlliance], searched: true }
		});
		const img = container.querySelector('img.portrait') as HTMLImageElement;
		expect(img.src).toContain('/alliances/99000001/logo');
	});
});

describe('MemberPicker 3-char guard', () => {
	it('disables the search button below 3 chars and prompts for more', async () => {
		render(MemberPicker, { props: {} });
		const input = screen.getByRole('searchbox');
		const button = screen.getByRole('button', { name: 'search' });

		await fireEvent.input(input, { target: { value: 'wa' } });
		expect(button).toBeDisabled();
		expect(screen.getByText(/at least 3 characters/i)).toBeInTheDocument();
	});

	it('enables the search button at 3 chars', async () => {
		render(MemberPicker, { props: {} });
		const input = screen.getByRole('searchbox');
		await fireEvent.input(input, { target: { value: 'wasp' } });
		expect(screen.getByRole('button', { name: 'search' })).not.toBeDisabled();
	});
});

describe('MemberPicker search scope', () => {
	it('offers character, corporation, alliance, and any radios in the search form', () => {
		render(MemberPicker, { props: {} });
		for (const label of ['Character', 'Corporation', 'Alliance', 'Any']) {
			expect(screen.getByRole('radio', { name: label })).toBeInTheDocument();
		}
	});

	it('defaults the scope to "any" and submits it as the scope field', async () => {
		const { container } = render(MemberPicker, { props: {} });
		const anyRadio = screen.getByRole('radio', { name: 'Any' }) as HTMLInputElement;
		expect(anyRadio.checked).toBe(true);
		expect(anyRadio.name).toBe('scope');
		// All scope radios live in the search form so the chosen scope submits with it.
		const form = container.querySelector('form.search-form') as HTMLFormElement;
		expect(form.querySelectorAll('input[name=scope]')).toHaveLength(4);
	});

	it('lets the user narrow the scope to a single category', async () => {
		render(MemberPicker, { props: {} });
		const corpRadio = screen.getByRole('radio', { name: 'Corporation' }) as HTMLInputElement;
		await fireEvent.click(corpRadio);
		expect(corpRadio.checked).toBe(true);
		expect(corpRadio.value).toBe('corporation');
	});
});

describe('MemberPicker per-result permission state', () => {
	it('resets selected permissions when a new result set arrives', async () => {
		const { container, rerender } = render(MemberPicker, {
			props: { characters: [aCharacter], searched: true }
		});

		// Pick a non-default permission on the first result set.
		const select = row(container, 'Wasp 223').querySelector('select') as HTMLSelectElement;
		await fireEvent.change(select, { target: { value: 'admin' } });
		expect(select.value).toBe('admin');

		// A new search returns a different character; the picker must not carry the
		// previous selection over — the new row defaults back to 'read'.
		const other = { id: 'char-uuid-2', eve_character_id: 8, name: 'Hornet 9' };
		await rerender({ characters: [other], searched: true });

		const newSelect = row(container, 'Hornet 9').querySelector('select') as HTMLSelectElement;
		expect(newSelect.value).toBe('read');
	});

	it('defaults to read for every fresh result row', () => {
		const { container } = render(MemberPicker, {
			props: { characters: [aCharacter], corporations: [aCorp], searched: true }
		});
		const charSelect = row(container, 'Wasp 223').querySelector('select') as HTMLSelectElement;
		const corpSelect = row(container, 'Test Corp').querySelector('select') as HTMLSelectElement;
		expect(charSelect.value).toBe('read');
		expect(corpSelect.value).toBe('read');
	});
});

describe('MemberPicker unavailable vs empty', () => {
	it('renders a distinct "search unavailable" notice', () => {
		render(MemberPicker, { props: { unavailable: true, searched: true } });
		expect(screen.getByText(/unavailable/i)).toBeInTheDocument();
		expect(screen.queryByText(/no matches/i)).toBeNull();
	});

	it('renders "no matches" when the search ran and matched nothing', () => {
		render(MemberPicker, { props: { unavailable: false, searched: true } });
		expect(screen.getByText(/no matches/i)).toBeInTheDocument();
		expect(screen.queryByText(/unavailable/i)).toBeNull();
	});

	it('shows neither state before a search has run', () => {
		render(MemberPicker, { props: { searched: false } });
		expect(screen.queryByText(/no matches/i)).toBeNull();
		expect(screen.queryByText(/unavailable/i)).toBeNull();
	});
});
