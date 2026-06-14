<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type {
		AdminAccountDto,
		AdminAccountCharacterDto,
		HardDeletePreviewDto,
		TokenStatus
	} from '$lib/api';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// Hard-delete flow state. `deleteTarget` is the account whose deletion is
	// being confirmed; `deletePreview` holds the fetched blast radius (null until
	// the preview action returns). `deleting` guards a double-submit.
	let deleteOpen = $state(false);
	let deleteTarget = $state<AdminAccountDto | null>(null);
	let deletePreview = $state<HardDeletePreviewDto | null>(null);
	let deleteError = $state(false);
	let previewFormEl = $state<HTMLFormElement | null>(null);
	let previewAccountInput = $state<HTMLInputElement | null>(null);
	let deleteFormEl = $state<HTMLFormElement | null>(null);
	let deleteAccountInput = $state<HTMLInputElement | null>(null);
	let pendingPreviewId = $state<string | null>(null);

	function openDelete(account: AdminAccountDto) {
		deleteTarget = account;
		deletePreview = null;
		deleteError = false;
		pendingPreviewId = account.id;
		deleteOpen = true;
		// Write the id imperatively right before submitting so the posted value is
		// never a stale render (the reactive input value may not have flushed to
		// the DOM yet). Fetch the preview; the dialog opens immediately and shows a
		// loading line until the counts arrive. requestSubmit is unavailable in
		// some test environments (jsdom) — guard so the dialog still opens there.
		if (previewAccountInput) previewAccountInput.value = account.id;
		try {
			previewFormEl?.requestSubmit();
		} catch {
			// no-op — the dialog stays in its loading state.
		}
	}

	function submitDelete() {
		if (deleteTarget && deleteAccountInput) deleteAccountInput.value = deleteTarget.id;
		try {
			deleteFormEl?.requestSubmit();
		} catch {
			deleteError = true;
		}
	}

	function closeDelete() {
		deleteOpen = false;
		deleteTarget = null;
		deletePreview = null;
		deleteError = false;
		pendingPreviewId = null;
	}

	type StatusFilter = 'all' | 'problems' | 'expired' | 'transferred';
	type SortColumn = 'account' | 'status' | 'admin' | 'issues' | 'created';
	type SortDir = 'asc' | 'desc';

	let textFilter = $state('');
	let statusFilter = $state<StatusFilter>('all');
	let sort = $state<{ column: SortColumn; dir: SortDir }>({ column: 'issues', dir: 'desc' });
	let expanded = $state<Set<string>>(new Set());

	function isProblem(status: TokenStatus): boolean {
		return status !== 'active';
	}

	// The main character's name identifies an account; fall back to the first
	// character by name, then the denormalized last-known main (so an orphaned,
	// zero-character account stays nameable), then a generic label.
	function accountLabel(account: AdminAccountDto): string {
		const main = account.characters.find((c) => c.is_main);
		const named = main ?? [...account.characters].sort((a, b) => a.name.localeCompare(b.name))[0];
		return (
			named?.name ?? account.last_known_main_character_name ?? m.admin_characters_no_account()
		);
	}

	function isOrphaned(account: AdminAccountDto): boolean {
		return account.status === 'orphaned';
	}

	function altCount(account: AdminAccountDto): number {
		return Math.max(0, account.characters.length - 1);
	}

	function countStatus(account: AdminAccountDto, status: TokenStatus): number {
		return account.characters.filter((c) => c.token_status === status).length;
	}

	function problemCount(account: AdminAccountDto): number {
		return account.characters.filter((c) => isProblem(c.token_status)).length;
	}

	// Worst token state present on the account, for sorting by issue severity.
	// owner_mismatch (transferred) is ranked above expired above clean.
	function issueSeverity(account: AdminAccountDto): number {
		if (countStatus(account, 'owner_mismatch') > 0) return 3;
		if (countStatus(account, 'expired') > 0) return 2;
		return 0;
	}

	function tokenLabel(status: TokenStatus): string {
		if (status === 'active') return m.admin_characters_token_active();
		if (status === 'owner_mismatch') return m.admin_characters_token_transferred();
		return m.admin_characters_token_expired();
	}

	function matchesText(account: AdminAccountDto, needle: string): boolean {
		const q = needle.trim().toLowerCase();
		if (q === '') return true;
		return account.characters.some((c) => c.name.toLowerCase().includes(q));
	}

	function matchesStatus(account: AdminAccountDto, filter: StatusFilter): boolean {
		if (filter === 'all') return true;
		if (filter === 'problems') return account.characters.some((c) => isProblem(c.token_status));
		if (filter === 'expired') return countStatus(account, 'expired') > 0;
		return countStatus(account, 'owner_mismatch') > 0;
	}

	let filtered = $derived(
		data.accounts.filter(
			(a) => matchesText(a, textFilter) && matchesStatus(a, statusFilter)
		)
	);

	let rows = $derived.by<AdminAccountDto[]>(() => {
		const dir = sort.dir === 'asc' ? 1 : -1;
		const by = (a: AdminAccountDto, b: AdminAccountDto): number => {
			switch (sort.column) {
				case 'account':
					return accountLabel(a).localeCompare(accountLabel(b));
				case 'status':
					return a.status.localeCompare(b.status);
				case 'admin':
					return Number(a.is_server_admin) - Number(b.is_server_admin);
				case 'created':
					return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
				case 'issues':
					return issueSeverity(a) - issueSeverity(b) || problemCount(a) - problemCount(b);
			}
		};
		// Stable tiebreak on account label keeps order deterministic.
		return [...filtered].sort((a, b) => by(a, b) * dir || accountLabel(a).localeCompare(accountLabel(b)));
	});

	function toggleSort(column: SortColumn) {
		if (sort.column === column) {
			sort = { column, dir: sort.dir === 'asc' ? 'desc' : 'asc' };
		} else {
			sort = { column, dir: 'asc' };
		}
	}

	function ariaSort(column: SortColumn): 'ascending' | 'descending' | 'none' {
		if (sort.column !== column) return 'none';
		return sort.dir === 'asc' ? 'ascending' : 'descending';
	}

	function toggleExpand(id: string) {
		const next = new Set(expanded);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expanded = next;
	}

	// Main first, then characters needing attention, then by name.
	function sortedCharacters(account: AdminAccountDto): AdminAccountCharacterDto[] {
		return [...account.characters].sort((a, b) => {
			if (a.is_main !== b.is_main) return Number(b.is_main) - Number(a.is_main);
			if (isProblem(a.token_status) !== isProblem(b.token_status)) {
				return Number(isProblem(b.token_status)) - Number(isProblem(a.token_status));
			}
			return a.name.localeCompare(b.name);
		});
	}
</script>

<svelte:head>
	<title>{m.admin_characters_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_characters_heading()}</h1>

<section class="panel">
	<p class="intro">{m.admin_characters_intro()}</p>

	<div class="controls">
		<div class="filter-field">
			<input
				type="search"
				class="filter-input"
				placeholder={m.admin_characters_filter_placeholder()}
				aria-label={m.admin_characters_filter_aria()}
				autocomplete="off"
				bind:value={textFilter}
			/>
			{#if textFilter !== ''}
				<button
					type="button"
					class="filter-clear"
					aria-label={m.admin_characters_filter_clear()}
					onclick={() => (textFilter = '')}
				>
					×
				</button>
			{/if}
		</div>
		<div class="chips" role="group" aria-label={m.admin_characters_col_status()}>
			<button
				type="button"
				class="chip"
				class:active={statusFilter === 'all'}
				onclick={() => (statusFilter = 'all')}
			>
				{m.admin_characters_filter_all()}
			</button>
			<button
				type="button"
				class="chip"
				class:active={statusFilter === 'problems'}
				onclick={() => (statusFilter = 'problems')}
			>
				{m.admin_characters_filter_problems()}
			</button>
			<button
				type="button"
				class="chip"
				class:active={statusFilter === 'expired'}
				onclick={() => (statusFilter = 'expired')}
			>
				{m.admin_characters_filter_expired()}
			</button>
			<button
				type="button"
				class="chip"
				class:active={statusFilter === 'transferred'}
				onclick={() => (statusFilter = 'transferred')}
			>
				{m.admin_characters_filter_transferred()}
			</button>
		</div>
	</div>

	{#if data.accounts.length === 0}
		<p class="empty" role="status">{m.admin_characters_empty()}</p>
	{:else if rows.length === 0}
		<p class="empty" role="status">{m.admin_characters_no_match()}</p>
	{:else}
		<table class="grid">
			<thead>
				<tr>
					<th class="expand-col" aria-hidden="true"></th>
					<th aria-sort={ariaSort('account')}>
						<button type="button" class="sort" onclick={() => toggleSort('account')}>
							{m.admin_characters_col_account()}
						</button>
					</th>
					<th aria-sort={ariaSort('status')}>
						<button type="button" class="sort" onclick={() => toggleSort('status')}>
							{m.admin_characters_col_status()}
						</button>
					</th>
					<th aria-sort={ariaSort('admin')}>
						<button type="button" class="sort" onclick={() => toggleSort('admin')}>
							{m.admin_characters_col_admin()}
						</button>
					</th>
					<th>{m.admin_characters_col_alts()}</th>
					<th aria-sort={ariaSort('issues')}>
						<button type="button" class="sort" onclick={() => toggleSort('issues')}>
							{m.admin_characters_col_issues()}
						</button>
					</th>
					<th aria-sort={ariaSort('created')}>
						<button type="button" class="sort" onclick={() => toggleSort('created')}>
							{m.admin_characters_col_created()}
						</button>
					</th>
					<th>{m.admin_characters_col_actions()}</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as account (account.id)}
					{@const expanded_ = expanded.has(account.id)}
					{@const expiredCount = countStatus(account, 'expired')}
					{@const transferredCount = countStatus(account, 'owner_mismatch')}
					<tr class="account-row" class:expanded={expanded_}>
						<td class="expand-col">
							<button
								type="button"
								class="expand"
								aria-expanded={expanded_}
								aria-label={expanded_
									? m.admin_characters_collapse({ name: accountLabel(account) })
									: m.admin_characters_expand({ name: accountLabel(account) })}
								onclick={() => toggleExpand(account.id)}
							>
								{expanded_ ? '▾' : '▸'}
							</button>
						</td>
						<td class="account-cell">
							{accountLabel(account)}
							{#if isOrphaned(account)}
								<span class="badge-orphaned" title={m.admin_characters_orphaned_hint({ name: accountLabel(account) })}>
									{m.admin_characters_orphaned_label()}
								</span>
							{/if}
						</td>
						<td class="muted">{account.status}</td>
						<td>
							{#if account.is_server_admin}
								<span class="badge-admin">{m.admin_characters_admin_yes()}</span>
							{:else}
								<span class="muted">—</span>
							{/if}
						</td>
						<td class="muted">
							{#if altCount(account) > 0}
								{m.admin_characters_alt_count({ count: altCount(account) })}
							{:else}
								—
							{/if}
						</td>
						<td class="issues-cell">
							{#if expiredCount === 0 && transferredCount === 0}
								<span class="muted">{m.admin_characters_issues_none()}</span>
							{:else}
								{#if transferredCount > 0}
									<span class="issue" data-state="owner_mismatch">
										<span class="dot" aria-hidden="true"></span>
										<span>{m.admin_characters_issues_transferred({ count: transferredCount })}</span>
									</span>
								{/if}
								{#if expiredCount > 0}
									<span class="issue" data-state="expired">
										<span class="dot" aria-hidden="true"></span>
										<span>{m.admin_characters_issues_expired({ count: expiredCount })}</span>
									</span>
								{/if}
							{/if}
						</td>
						<td class="muted">{new Date(account.created_at).toLocaleDateString()}</td>
						<td class="actions-cell">
							<button
								type="button"
								class="delete-btn"
								aria-label={m.admin_characters_delete_aria({ name: accountLabel(account) })}
								onclick={() => openDelete(account)}
							>
								{m.admin_characters_delete()}
							</button>
						</td>
					</tr>
					{#if expanded_}
						<tr class="detail-row">
							<td></td>
							<td colspan="7">
								<table class="char-table">
									<thead>
										<tr>
											<th>{m.admin_characters_dialog_col_character()}</th>
											<th>{m.admin_characters_dialog_col_status()}</th>
										</tr>
									</thead>
									<tbody>
										{#each sortedCharacters(account) as character (character.eve_character_id)}
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
							</td>
						</tr>
					{/if}
				{/each}
			</tbody>
		</table>
	{/if}
</section>

<!-- Preview form: fetches the blast radius for the targeted account. The
     account id is bound to the current delete target. -->
<form
	bind:this={previewFormEl}
	method="POST"
	action="?/preview"
	use:enhance={() =>
		async ({ result }) => {
			if (result.type === 'success' && result.data?.action === 'preview') {
				// Only apply if this is still the account we are confirming.
				if (result.data.accountId === pendingPreviewId) {
					deletePreview = result.data.preview as HardDeletePreviewDto;
				}
			} else if (result.type === 'failure') {
				deleteError = true;
			}
		}}
>
	<input bind:this={previewAccountInput} type="hidden" name="account_id" value="" />
</form>

<!-- Delete form: dispatched on confirm. On success refreshes the grid so the
     deleted account disappears. -->
<form
	bind:this={deleteFormEl}
	method="POST"
	action="?/delete"
	use:enhance={() =>
		async ({ result }) => {
			if (result.type === 'success' && result.data?.action === 'delete') {
				closeDelete();
				await invalidateAll();
			} else if (result.type === 'failure') {
				deleteError = true;
			}
		}}
>
	<input bind:this={deleteAccountInput} type="hidden" name="account_id" value="" />
</form>

<ConfirmDialog
	open={deleteOpen}
	tone="danger"
	onCancel={closeDelete}
	onConfirm={submitDelete}
>
	{#snippet title()}
		{m.admin_characters_delete_title({ name: deleteTarget ? accountLabel(deleteTarget) : '' })}
	{/snippet}
	{#snippet body()}
		{#if deleteError}
			<span class="dialog-error" role="alert">{m.admin_characters_delete_error()}</span>
		{:else if deletePreview === null}
			<span role="status">{m.admin_characters_delete_loading()}</span>
		{:else}
			{m.admin_characters_delete_intro()}
			{' '}
			{m.admin_characters_delete_removed({
				characters: deletePreview.characters,
				sessions: deletePreview.sessions,
				api_keys: deletePreview.api_keys
			})}
			{' '}
			{m.admin_characters_delete_unowned({
				maps: deletePreview.owned_maps,
				acls: deletePreview.owned_acls
			})}
			{' '}
			{m.admin_characters_delete_audit_preserved()}
		{/if}
	{/snippet}
	{#snippet confirmLabel()}{m.admin_characters_delete_confirm()}{/snippet}
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
	.intro {
		margin: 0 0 16px;
		font-size: 0.75rem;
		color: var(--slate-400);
	}

	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 12px;
		margin-bottom: 16px;
	}
	.filter-field {
		position: relative;
		flex: 1;
		min-width: 200px;
		display: flex;
	}
	.filter-input {
		flex: 1;
		padding: 8px 32px 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.filter-input:focus {
		outline: none;
		border-color: var(--sky);
	}
	/* Suppress the native search clear so it doesn't duplicate our button. */
	.filter-input::-webkit-search-cancel-button {
		appearance: none;
	}
	.filter-clear {
		position: absolute;
		top: 50%;
		right: 6px;
		transform: translateY(-50%);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.25rem;
		height: 1.25rem;
		background: transparent;
		border: 0;
		padding: 0;
		color: var(--slate-500);
		font-size: 1.125rem;
		line-height: 1;
		cursor: pointer;
	}
	.filter-clear:hover {
		color: var(--slate-100);
	}

	.chips {
		display: flex;
		align-items: center;
		gap: 6px;
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

	.grid {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}
	.grid th {
		text-align: left;
		padding: 8px 12px;
		color: var(--slate-500);
		font-weight: 500;
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		border-bottom: 1px solid var(--space-700);
	}
	.grid td {
		padding: 10px 12px;
		color: var(--slate-200);
		border-bottom: 1px solid var(--space-800);
	}
	.expand-col {
		width: 1%;
		padding-right: 0;
	}
	.account-row.expanded > td {
		border-bottom-color: transparent;
	}
	.account-cell {
		color: var(--slate-100);
	}
	.muted {
		color: var(--slate-500);
	}

	.sort {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: inherit;
		letter-spacing: inherit;
		text-transform: inherit;
		color: inherit;
		cursor: pointer;
	}
	.sort:hover {
		color: var(--slate-300);
	}
	.grid th[aria-sort='ascending'] .sort::after {
		content: '▲';
		font-size: 0.625rem;
		color: var(--sky);
	}
	.grid th[aria-sort='descending'] .sort::after {
		content: '▼';
		font-size: 0.625rem;
		color: var(--sky);
	}

	.expand {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 1.5rem;
		height: 1.5rem;
		background: transparent;
		border: 0;
		padding: 0;
		color: var(--slate-400);
		font-size: 1.125rem;
		line-height: 1;
		cursor: pointer;
	}
	.expand:hover {
		color: var(--slate-100);
	}

	.badge-admin {
		display: inline-flex;
		align-items: center;
		padding: 1px 6px;
		border-radius: 4px;
		background: rgba(56, 189, 248, 0.12);
		border: 1px solid rgba(56, 189, 248, 0.35);
		color: var(--sky);
		font-size: 0.625rem;
		font-weight: 500;
		letter-spacing: 0.05em;
	}

	.badge-orphaned {
		display: inline-flex;
		align-items: center;
		margin-left: 8px;
		padding: 1px 6px;
		border-radius: 4px;
		background: rgba(245, 158, 11, 0.1);
		border: 1px solid rgba(245, 158, 11, 0.35);
		color: var(--amber);
		font-size: 0.625rem;
		font-weight: 500;
		letter-spacing: 0.05em;
	}

	.actions-cell {
		white-space: nowrap;
	}
	.delete-btn {
		padding: 3px 10px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-400);
		font: inherit;
		font-size: 0.6875rem;
		cursor: pointer;
	}
	.delete-btn:hover {
		border-color: var(--red);
		color: var(--red);
	}
	.delete-btn:focus-visible {
		outline: 2px solid var(--red);
		outline-offset: 2px;
	}

	.dialog-error {
		color: var(--red);
	}

	.issues-cell {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 10px;
	}
	.issue {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 0.6875rem;
	}
	.issue .dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex-shrink: 0;
	}
	.issue[data-state='expired'] {
		color: var(--red);
	}
	.issue[data-state='expired'] .dot {
		background: var(--red);
	}
	.issue[data-state='owner_mismatch'] {
		color: var(--amber);
	}
	.issue[data-state='owner_mismatch'] .dot {
		background: var(--amber);
	}

	.detail-row > td {
		padding: 0 12px 12px;
		background: var(--space-950);
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
		border-bottom: 1px solid var(--space-800);
	}
	.char-table td {
		padding: 8px 12px;
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

	.empty {
		padding: 16px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}
</style>
