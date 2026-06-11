// Behavioural coverage of the audit browser's click-to-refine, chip removal,
// and search round-trip. The pure helpers (groupByDay, isSecurityEvent,
// catalogues) are tested in src/lib/audit.test.ts and not re-tested here.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { ComponentProps } from 'svelte';

const goto = vi.fn();
vi.mock('$app/navigation', () => ({
	goto: (...args: unknown[]) => goto(...args)
}));

const AuditPage = (await import('./+page.svelte')).default;
import type { AuditLogEntryDto } from '$lib/api';

function row(over: Partial<AuditLogEntryDto> = {}): AuditLogEntryDto {
	return {
		id: 'e1',
		occurred_at: new Date().toISOString(),
		actor_account_id: 'acc-1',
		actor_character_id: 123,
		actor_character_name: 'Wasp 223',
		event_type: 'acl_member_added',
		details: {},
		target_type: 'acl',
		target_id: 'acl-uuid',
		target_name: 'Corp ACL',
		...over
	};
}

function props(
	opts: { entries?: AuditLogEntryDto[]; filters?: Partial<Record<string, string>>; next_before?: string | null } = {}
): ComponentProps<typeof AuditPage> {
	return {
		data: {
			page: { entries: opts.entries ?? [row()], next_before: opts.next_before ?? null },
			filters: {
				event_type: '',
				actor: '',
				target_type: '',
				target_id: '',
				q: '',
				window: '7d',
				...opts.filters
			},
			pageLimit: 50
		}
	} as unknown as ComponentProps<typeof AuditPage>;
}

/** The URL string `goto` was last called with. */
function lastGotoUrl(): string {
	const call = goto.mock.calls.at(-1);
	return String(call?.[0] ?? '');
}

beforeEach(() => goto.mockReset());
afterEach(() => cleanup());

describe('audit click-to-refine', () => {
	it('clicking an Actor cell sets the actor filter (by account)', async () => {
		render(AuditPage, { props: props() });
		await tick();
		await fireEvent.click(screen.getByText('Wasp 223'));
		const url = lastGotoUrl();
		expect(url).toContain('actor=acc-1');
		expect(url).toContain('window=7d');
	});

	it('clicking an Event cell sets the event_type filter', async () => {
		const { container } = render(AuditPage, { props: props() });
		await tick();
		// The event value also appears as a <select> option; target the cell's
		// <code class="event"> specifically.
		const eventCell = container.querySelector('code.event');
		expect(eventCell?.textContent).toBe('acl_member_added');
		await fireEvent.click(eventCell!);
		expect(lastGotoUrl()).toContain('event_type=acl_member_added');
	});

	it('clicking a Target cell sets target_type + target_id', async () => {
		render(AuditPage, { props: props() });
		await tick();
		await fireEvent.click(screen.getByText('Corp ACL'));
		const url = lastGotoUrl();
		expect(url).toContain('target_type=acl');
		expect(url).toContain('target_id=acl-uuid');
	});

	it('replaces within a column: a new actor click swaps the existing actor filter', async () => {
		render(AuditPage, {
			props: props({
				entries: [row({ id: 'e2', actor_account_id: 'acc-2', actor_character_name: 'Other Pilot' })],
				filters: { actor: 'acc-1' }
			})
		});
		await tick();
		await fireEvent.click(screen.getByText('Other Pilot'));
		const url = lastGotoUrl();
		expect(url).toContain('actor=acc-2');
		expect(url).not.toContain('acc-1');
	});
});

describe('audit chips', () => {
	it('renders a chip per active filter and an actor chip worded as "account of"', () => {
		render(AuditPage, { props: props({ filters: { actor: 'acc-1', q: 'wasp' } }) });
		expect(screen.getByText('Account of acc-1')).toBeInTheDocument();
		expect(screen.getByText('Search: wasp')).toBeInTheDocument();
	});

	it('removing a chip navigates with that filter cleared', async () => {
		render(AuditPage, { props: props({ filters: { event_type: 'map_created' } }) });
		await tick();
		const remove = screen.getByLabelText('remove filter');
		await fireEvent.click(remove);
		expect(lastGotoUrl()).not.toContain('event_type');
	});
});

describe('audit search', () => {
	it('round-trips the search box value to the q query param on submit', async () => {
		render(AuditPage, { props: props() });
		await tick();
		const input = screen.getByLabelText('Search') as HTMLInputElement;
		await fireEvent.input(input, { target: { value: 'wasp' } });
		await fireEvent.submit(input.closest('form')!);
		expect(lastGotoUrl()).toContain('q=wasp');
	});
});
