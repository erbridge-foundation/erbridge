<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { AdminAccountDto, AdminAccountCharacterDto, TokenStatus } from '$lib/api';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

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
	// character by name, then a generic label for an account with no characters.
	function accountLabel(account: AdminAccountDto): string {
		const main = account.characters.find((c) => c.is_main);
		const named = main ?? [...account.characters].sort((a, b) => a.name.localeCompare(b.name))[0];
		return named?.name ?? m.admin_characters_no_account();
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
						<td class="account-cell">{accountLabel(account)}</td>
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
					</tr>
					{#if expanded_}
						<tr class="detail-row">
							<td></td>
							<td colspan="6">
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
