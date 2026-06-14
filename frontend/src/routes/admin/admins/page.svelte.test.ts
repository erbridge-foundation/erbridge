import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';
import AdminsPage from './+page.svelte';
import type { MeResponse, AdminAccountDto } from '$lib/api';

const AdminsPageComponent = AdminsPage;

// The logged-in admin. account.id drives the self-revoke warning.
const me: MeResponse = {
	account: { id: 'acc-self', status: 'active', is_server_admin: true, created_at: '2024-01-01T00:00:00Z' },
	characters: []
};

function adminAccount(id: string, name: string): AdminAccountDto {
	return {
		id,
		status: 'active',
		is_server_admin: true,
		created_at: '2024-01-01T00:00:00Z',
		last_known_main_character_name: name,
		characters: [
			{ eve_character_id: 1, name, is_main: true, token_status: 'active' }
		]
	};
}

function props(admins: AdminAccountDto[]): ComponentProps<typeof AdminsPageComponent> {
	return {
		data: { me, admins },
		form: null
	} as unknown as ComponentProps<typeof AdminsPageComponent>;
}

// Open the revoke dialog for the admin row labelled `name`.
async function openRevoke(name: string): Promise<void> {
	const row = screen.getByText(name).closest('tr') as HTMLElement;
	const revokeBtn = row.querySelector('button.danger') as HTMLButtonElement;
	await fireEvent.click(revokeBtn);
}

afterEach(() => cleanup());

describe('/admin/admins self-revoke warning', () => {
	it('shows the self-revoke warning when revoking your own account', async () => {
		render(AdminsPageComponent, {
			props: props([adminAccount('acc-self', 'SelfMain'), adminAccount('acc-other', 'OtherMain')])
		});
		await openRevoke('SelfMain');
		expect(
			screen.getByText(/you will lose your admin access/i)
		).toBeInTheDocument();
	});

	it('does not show the warning when revoking another account', async () => {
		render(AdminsPageComponent, {
			props: props([adminAccount('acc-self', 'SelfMain'), adminAccount('acc-other', 'OtherMain')])
		});
		await openRevoke('OtherMain');
		// The dialog is open (confirm control present) but carries no self-warning.
		expect(screen.getByRole('button', { name: /revoke admin/i })).toBeInTheDocument();
		expect(
			screen.queryByText(/you will lose your admin access/i)
		).not.toBeInTheDocument();
	});
});
