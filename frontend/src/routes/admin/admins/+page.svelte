<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type {
		AdminAccountDto,
		CharacterSearchResultDto
	} from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	// The logged-in admin, from the root layout load. Self-revoke is permitted by
	// the backend (guarded only by the last-admin rule), so we don't block it —
	// we warn when the row being revoked is the admin's own account.
	let me = $derived(data.me);
	function isSelf(account: AdminAccountDto | null): boolean {
		return !!me && !!account && account.id === me.account.id;
	}

	// The main character's name identifies an account; fall back to the first
	// character, or a generic label for an account with no characters.
	function accountLabel(account: AdminAccountDto): string {
		const main = account.characters.find((c) => c.is_main);
		const named = main ?? account.characters[0];
		return named?.name ?? m.admin_admins_no_account();
	}

	let searchResults = $derived(
		form?.action === 'search' && 'results' in form
			? (form.results as CharacterSearchResultDto[])
			: null
	);
	let searchQuery = $derived(
		form?.action === 'search' && 'query' in form ? (form.query as string) : ''
	);

	type FormError = { action: string; code: string; message: string; accountId?: string };
	let formError = $derived(
		form && 'code' in form ? (form as unknown as FormError) : null
	);

	// Revoke confirmation state (one modal at a time).
	let revokeState = $state<{ open: boolean; account: AdminAccountDto | null }>({
		open: false,
		account: null
	});
	let revokeForms = $state<Record<string, HTMLFormElement>>({});

	// Promote confirmation state for a chosen search result.
	let promoteState = $state<{ open: boolean; result: CharacterSearchResultDto | null }>({
		open: false,
		result: null
	});
	let promoteFormEl = $state<HTMLFormElement | null>(null);
	let promoteAccountInput = $state<HTMLInputElement | null>(null);
</script>

<svelte:head>
	<title>{m.admin_admins_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_admins_heading()}</h1>

<section class="panel">
	{#if data.admins.length === 0}
		<p class="empty" role="status">{m.admin_admins_empty()}</p>
	{:else}
		<table class="admin-table">
			<thead>
				<tr>
					<th>{m.admin_admins_col_account()}</th>
					<th>{m.admin_admins_col_created()}</th>
					<th class="actions-col">{m.admin_admins_col_actions()}</th>
				</tr>
			</thead>
			<tbody>
				{#each data.admins as account (account.id)}
					<tr>
						<td>{accountLabel(account)}</td>
						<td class="muted">{new Date(account.created_at).toLocaleDateString()}</td>
						<td class="actions-col">
							<form
								bind:this={revokeForms[account.id]}
								method="POST"
								action="?/revoke"
								use:enhance
							>
								<input type="hidden" name="account_id" value={account.id} />
								<button
									type="button"
									class="danger"
									onclick={() => (revokeState = { open: true, account })}
								>
									{m.admin_admins_revoke()}
								</button>
							</form>
							{#if formError?.action === 'revoke' && formError?.accountId === account.id}
								<p class="inline-error" role="alert" data-error-code={formError.code}>
									{formError.message}
								</p>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</section>

<section class="panel">
	<h2 class="panel-heading">{m.admin_admins_add_heading()}</h2>
	<p class="intro">{m.admin_admins_add_intro()}</p>

	<form method="POST" action="?/search" use:enhance class="search-form">
		<input
			type="search"
			name="q"
			placeholder={m.admin_admins_search_placeholder()}
			aria-label={m.admin_admins_search_aria()}
			autocomplete="off"
			value={searchQuery}
		/>
		<button type="submit" class="btn">{m.admin_admins_search_submit()}</button>
	</form>

	{#if formError?.action === 'search'}
		<p class="inline-error" role="alert" data-error-code={formError.code}>{formError.message}</p>
	{/if}
	{#if formError?.action === 'grant'}
		<p class="inline-error" role="alert" data-error-code={formError.code}>{formError.message}</p>
	{/if}

	{#if searchResults}
		{#if searchResults.length === 0}
			<p class="empty" role="status">{m.admin_admins_search_empty()}</p>
		{:else}
			<ul class="results">
				{#each searchResults as result (result.eve_character_id)}
					<li>
						<span class="result-name">{result.name}</span>
						{#if result.account_id}
							<button
								type="button"
								class="btn promote"
								onclick={() => (promoteState = { open: true, result })}
							>
								{m.admin_admins_promote()}
							</button>
						{:else}
							<span class="orphan">{m.admin_admins_search_orphan()}</span>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	{/if}

	<!-- Hidden grant form; submitted with the resolved account_id on confirm.
	     The id is written to the input imperatively right before requestSubmit so
	     the submitted value is never a stale render. -->
	<form bind:this={promoteFormEl} method="POST" action="?/grant" use:enhance class="hidden-form">
		<input bind:this={promoteAccountInput} type="hidden" name="account_id" value="" />
	</form>
</section>

<!-- Revoke confirmation -->
<ConfirmDialog
	open={revokeState.open}
	tone="danger"
	onCancel={() => (revokeState = { open: false, account: null })}
	onConfirm={() => {
		if (revokeState.account) {
			revokeForms[revokeState.account.id]?.requestSubmit();
		}
		revokeState = { open: false, account: null };
	}}
>
	{#snippet title()}{m.admin_admins_revoke_title({
			name: revokeState.account ? accountLabel(revokeState.account) : ''
		})}{/snippet}
	{#snippet body()}{m.admin_admins_revoke_body()}{#if isSelf(revokeState.account)}<span
				class="self-revoke-warning"
				role="alert">{m.admin_admins_revoke_self_warning()}</span
			>{/if}{/snippet}
	{#snippet confirmLabel()}{m.admin_admins_revoke_confirm()}{/snippet}
</ConfirmDialog>

<!-- Promote (grant) confirmation -->
<ConfirmDialog
	open={promoteState.open}
	tone="danger"
	onCancel={() => (promoteState = { open: false, result: null })}
	onConfirm={() => {
		if (promoteState.result?.account_id && promoteAccountInput) {
			promoteAccountInput.value = promoteState.result.account_id;
			promoteFormEl?.requestSubmit();
		}
		promoteState = { open: false, result: null };
	}}
>
	{#snippet title()}{m.admin_admins_promote_title({
			name: promoteState.result?.name ?? ''
		})}{/snippet}
	{#snippet body()}{m.admin_admins_promote_body()}{/snippet}
	{#snippet confirmLabel()}{m.admin_admins_promote_confirm()}{/snippet}
</ConfirmDialog>

<style>
	.page-heading {
		margin: 0 0 16px;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-500);
	}

	.panel {
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		padding: 20px;
		margin-bottom: 24px;
	}
	.panel-heading {
		margin: 0 0 8px;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	.intro {
		margin: 0 0 16px;
		font-size: 0.75rem;
		color: var(--slate-400);
	}

	.admin-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}
	.admin-table th {
		text-align: left;
		padding: 8px 12px;
		color: var(--slate-500);
		font-weight: 500;
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border-bottom: 1px solid var(--space-700);
	}
	.admin-table td {
		padding: 10px 12px;
		color: var(--slate-200);
		border-bottom: 1px solid var(--space-800);
	}
	.actions-col {
		text-align: right;
		width: 1%;
		white-space: nowrap;
	}
	.muted {
		color: var(--slate-500);
	}

	.search-form {
		display: flex;
		gap: 8px;
		margin-bottom: 16px;
	}
	.search-form input {
		flex: 1;
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.search-form input:focus {
		outline: none;
		border-color: var(--sky);
	}

	.results {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.results li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-800);
		border-radius: 4px;
	}
	.result-name {
		font-size: 0.8125rem;
		color: var(--slate-100);
	}
	.orphan {
		font-size: 0.6875rem;
		color: var(--slate-500);
	}

	.btn {
		display: inline-flex;
		align-items: center;
		padding: 8px 14px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
		white-space: nowrap;
	}
	.btn:hover {
		background: var(--space-700);
	}
	.btn.promote {
		padding: 4px 10px;
	}

	button.danger {
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: 0.75rem;
		color: var(--slate-400);
		cursor: pointer;
	}
	button.danger:hover {
		color: var(--red);
	}

	.hidden-form {
		display: none;
	}

	.empty {
		padding: 16px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}

	.inline-error {
		margin: 8px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}

	/* Self-revoke warning inside the confirm dialog body. display:block keeps it a
	   valid inline child of the body <p> while reading as its own banner. */
	.self-revoke-warning {
		display: block;
		margin-top: 12px;
		padding: 8px 12px;
		background: rgba(245, 158, 11, 0.08);
		border: 1px solid var(--amber);
		border-radius: 4px;
		color: var(--amber);
	}
</style>
