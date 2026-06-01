<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import type {
		AdminAccountDto,
		AdminAccountCharacterDto,
		CharacterSearchResultDto,
		TokenStatus
	} from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	let searchResults = $derived(
		form?.action === 'search' && 'results' in form
			? (form.results as CharacterSearchResultDto[])
			: null
	);
	let searchQuery = $derived(
		form?.action === 'search' && 'query' in form ? (form.query as string) : ''
	);

	type FormError = { action: string; code: string; message: string };
	let formError = $derived(form && 'code' in form ? (form as unknown as FormError) : null);

	// Index accounts by id so a search result can resolve to its full account
	// (with every character + token_status) for the inspect dialog.
	let accountsById = $derived(
		new Map(data.accounts.map((a: AdminAccountDto) => [a.id, a]))
	);

	// The inspect dialog: holds the selected account and the character that was
	// clicked (for the dialog title).
	let inspect = $state<{ open: boolean; account: AdminAccountDto | null; name: string }>({
		open: false,
		account: null,
		name: ''
	});

	// Filter within the dialog: 'all' or only characters needing attention
	// (token_expired / owner_mismatch).
	let filter = $state<'all' | 'problems'>('all');

	function isProblem(status: TokenStatus): boolean {
		return status !== 'active';
	}

	let dialogCharacters = $derived.by<AdminAccountCharacterDto[]>(() => {
		const chars = inspect.account?.characters ?? [];
		const list = filter === 'problems' ? chars.filter((c) => isProblem(c.token_status)) : chars;
		// Main first, then characters needing attention, then by name.
		return [...list].sort((a, b) => {
			if (a.is_main !== b.is_main) return Number(b.is_main) - Number(a.is_main);
			if (isProblem(a.token_status) !== isProblem(b.token_status)) {
				return Number(isProblem(b.token_status)) - Number(isProblem(a.token_status));
			}
			return a.name.localeCompare(b.name);
		});
	});

	function tokenLabel(status: TokenStatus): string {
		if (status === 'active') return m.admin_characters_token_active();
		if (status === 'owner_mismatch') return m.admin_characters_token_transferred();
		return m.admin_characters_token_expired();
	}

	function openInspect(result: CharacterSearchResultDto) {
		if (!result.account_id) return;
		const account = accountsById.get(result.account_id);
		if (!account) return;
		filter = 'all';
		inspect = { open: true, account, name: result.name };
	}

	function closeInspect() {
		inspect = { open: false, account: null, name: '' };
	}
</script>

<svelte:head>
	<title>{m.admin_characters_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_characters_heading()}</h1>

<section class="panel">
	<p class="intro">{m.admin_characters_intro()}</p>

	<form method="POST" action="?/search" use:enhance class="search-form">
		<input
			type="search"
			name="q"
			placeholder={m.admin_characters_search_placeholder()}
			aria-label={m.admin_characters_search_aria()}
			autocomplete="off"
			value={searchQuery}
		/>
		<button type="submit" class="btn">{m.admin_characters_search_submit()}</button>
	</form>

	{#if formError?.action === 'search'}
		<p class="inline-error" role="alert" data-error-code={formError.code}>{formError.message}</p>
	{/if}

	{#if searchResults}
		{#if searchResults.length === 0}
			<p class="empty" role="status">{m.admin_characters_search_empty()}</p>
		{:else}
			<ul class="results">
				{#each searchResults as result (result.eve_character_id)}
					<li>
						<span class="result-name">{result.name}</span>
						{#if result.account_id}
							<button type="button" class="btn inspect" onclick={() => openInspect(result)}>
								{m.admin_characters_inspect()}
							</button>
						{:else}
							<span class="orphan">{m.admin_characters_search_orphan()}</span>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
</section>

{#if inspect.open && inspect.account}
	<!-- Informational inspect dialog (read-only account view). Backdrop close
	     via pointer-down + Escape, matching ConfirmDialog's pattern. -->
	<div
		class="modal-backdrop"
		role="presentation"
		onpointerdown={(e) => e.target === e.currentTarget && closeInspect()}
		onkeydown={(e) => e.key === 'Escape' && closeInspect()}
	>
		<div
			class="modal"
			role="dialog"
			aria-modal="true"
			aria-label={m.admin_characters_dialog_title({ name: inspect.name })}
			tabindex="-1"
		>
			<div class="modal-header">
				<h2>{m.admin_characters_dialog_title({ name: inspect.name })}</h2>
				<button type="button" class="close" aria-label={m.admin_characters_dialog_close()} onclick={closeInspect}>
					×
				</button>
			</div>

			{#if inspect.account.characters.length === 0}
				<p class="empty" role="status">{m.admin_characters_dialog_no_account()}</p>
			{:else}
				<div class="filter">
					<span class="filter-label">{m.admin_characters_filter_label()}</span>
					<button
						type="button"
						class="chip"
						class:active={filter === 'all'}
						onclick={() => (filter = 'all')}
					>
						{m.admin_characters_filter_all()}
					</button>
					<button
						type="button"
						class="chip"
						class:active={filter === 'problems'}
						onclick={() => (filter = 'problems')}
					>
						{m.admin_characters_filter_problems()}
					</button>
				</div>

				<table class="char-table">
					<thead>
						<tr>
							<th>{m.admin_characters_dialog_col_character()}</th>
							<th>{m.admin_characters_dialog_col_status()}</th>
						</tr>
					</thead>
					<tbody>
						{#each dialogCharacters as character (character.eve_character_id)}
							<tr>
								<td>
									<span class="char-name">{character.name}</span>
									{#if character.is_main}
										<span class="badge-main">{m.admin_characters_badge_main()}</span>
									{/if}
								</td>
								<td>
									<span class="token-status" data-state={character.token_status}>
										<span class="dot" aria-hidden="true"></span>
										<span>{tokenLabel(character.token_status)}</span>
									</span>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
		</div>
	</div>
{/if}

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
	.intro {
		margin: 0 0 16px;
		font-size: 0.75rem;
		color: var(--slate-400);
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
	.btn.inspect {
		padding: 4px 10px;
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

	/* Inspect dialog */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
		z-index: 50;
	}
	.modal {
		width: 100%;
		max-width: 480px;
		max-height: 80vh;
		overflow: auto;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 8px;
		padding: 20px;
	}
	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin-bottom: 16px;
	}
	.modal-header h2 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	.close {
		background: transparent;
		border: 0;
		color: var(--slate-400);
		font-size: 1.25rem;
		line-height: 1;
		cursor: pointer;
		padding: 0 4px;
	}
	.close:hover {
		color: var(--slate-100);
	}

	.filter {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-bottom: 12px;
	}
	.filter-label {
		font-size: 0.6875rem;
		color: var(--slate-500);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-right: 2px;
	}
	.chip {
		padding: 3px 10px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 999px;
		color: var(--slate-400);
		font: inherit;
		font-size: 0.6875rem;
		cursor: pointer;
	}
	.chip:hover {
		color: var(--slate-200);
	}
	.chip.active {
		color: var(--sky);
		border-color: var(--sky);
	}

	.char-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}
	.char-table th {
		text-align: left;
		padding: 8px 12px;
		color: var(--slate-500);
		font-weight: 500;
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border-bottom: 1px solid var(--space-700);
	}
	.char-table td {
		padding: 10px 12px;
		color: var(--slate-200);
		border-bottom: 1px solid var(--space-800);
	}
	.char-name {
		color: var(--slate-100);
	}
	.badge-main {
		display: inline-flex;
		align-items: center;
		margin-left: 8px;
		padding: 1px 6px;
		border-radius: 4px;
		background: rgba(56, 189, 248, 0.12);
		border: 1px solid rgba(56, 189, 248, 0.35);
		color: var(--sky);
		font-size: 0.625rem;
		font-weight: 500;
		letter-spacing: 0.05em;
	}

	.token-status {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.token-status .dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex-shrink: 0;
	}
	.token-status[data-state='active'] .dot {
		background: var(--emerald);
	}
	.token-status[data-state='expired'] .dot {
		background: var(--red);
	}
	.token-status[data-state='expired'] {
		color: var(--red);
	}
	.token-status[data-state='owner_mismatch'] .dot {
		background: var(--amber);
	}
	.token-status[data-state='owner_mismatch'] {
		color: var(--amber);
	}
</style>
