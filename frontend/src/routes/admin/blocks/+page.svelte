<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type {
		BlockedCharacterDto,
		CharacterSearchResultDto,
		EsiCharacterSearchResultDto
	} from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	const MIN_SEARCH_LEN = 3;

	function blockLabel(block: BlockedCharacterDto): string {
		return block.character_name ?? m.admin_blocks_unknown_name();
	}

	type FormShape = {
		action: string;
		code?: string;
		message?: string;
		query?: string;
		results?: (CharacterSearchResultDto | EsiCharacterSearchResultDto)[];
		unavailable?: boolean;
		eve_character_id?: number;
		corporation_name?: string | null;
		eveCharacterId?: number;
	};
	let f = $derived(form as FormShape | null);

	// The current search box value (mirrors the last submitted query so the input
	// keeps its text across the enhance round-trip).
	let query = $state('');

	let localResults = $derived(
		f?.action === 'search' && f.results
			? (f.results as CharacterSearchResultDto[])
			: null
	);
	let esiResults = $derived(
		f?.action === 'esiSearch' && f.results
			? (f.results as EsiCharacterSearchResultDto[])
			: null
	);
	let esiUnavailable = $derived(f?.action === 'esiSearch' ? Boolean(f.unavailable) : false);

	let searchError = $derived(
		f && (f.action === 'search' || f.action === 'esiSearch') && f.code && !f.results
			? f.message
			: null
	);

	// Pending block selection: holds the chosen character while we look up its
	// corp and confirm. `corpLookup` returns the corp; we then open the dialog.
	type Selected = { eve_character_id: number; name: string };
	let selected = $state<Selected | null>(null);
	let confirmOpen = $state(false);
	let reason = $state('');
	let blockFormEl = $state<HTMLFormElement | null>(null);
	let blockIdInput = $state<HTMLInputElement | null>(null);
	let blockReasonInput = $state<HTMLInputElement | null>(null);

	// When a corpLookup result returns and matches the pending selection, open the
	// confirm dialog enriched with the corp name.
	let corpName = $state<string | null>(null);
	$effect(() => {
		if (
			f?.action === 'corpLookup' &&
			selected &&
			f.eve_character_id === selected.eve_character_id
		) {
			corpName = f.corporation_name ?? null;
			confirmOpen = true;
		}
	});

	function chooseCharacter(eve_character_id: number, name: string) {
		selected = { eve_character_id, name };
		corpName = null;
	}

	// Unblock confirmation state (one modal at a time).
	let unblockState = $state<{ open: boolean; block: BlockedCharacterDto | null }>({
		open: false,
		block: null
	});
	let unblockForms = $state<Record<number, HTMLFormElement>>({});

	let unblockError = $derived(
		f?.action === 'unblock' && f.code ? { code: f.code, message: f.message, id: f.eveCharacterId } : null
	);
</script>

<svelte:head>
	<title>{m.admin_blocks_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_blocks_heading()}</h1>

<section class="panel">
	{#if data.blocks.length === 0}
		<p class="empty" role="status">{m.admin_blocks_empty()}</p>
	{:else}
		<table class="admin-table">
			<thead>
				<tr>
					<th>{m.admin_blocks_col_character()}</th>
					<th>{m.admin_blocks_col_corporation()}</th>
					<th>{m.admin_blocks_col_reason()}</th>
					<th>{m.admin_blocks_col_blocked_at()}</th>
					<th class="actions-col">{m.admin_blocks_col_actions()}</th>
				</tr>
			</thead>
			<tbody>
				{#each data.blocks as block (block.eve_character_id)}
					<tr>
						<td>{blockLabel(block)}</td>
						<td class="muted">{block.corporation_name ?? '—'}</td>
						<td class="muted">{block.reason ?? '—'}</td>
						<td class="muted">{new Date(block.blocked_at).toLocaleDateString()}</td>
						<td class="actions-col">
							<form
								bind:this={unblockForms[block.eve_character_id]}
								method="POST"
								action="?/unblock"
								use:enhance
							>
								<input type="hidden" name="eve_character_id" value={block.eve_character_id} />
								<button
									type="button"
									class="danger"
									onclick={() => (unblockState = { open: true, block })}
								>
									{m.admin_blocks_unblock()}
								</button>
							</form>
							{#if unblockError?.id === block.eve_character_id}
								<p class="inline-error" role="alert" data-error-code={unblockError.code}>
									{unblockError.message}
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
	<h2 class="panel-heading">{m.admin_blocks_add_heading()}</h2>
	<p class="intro">{m.admin_blocks_add_intro()}</p>

	<form method="POST" action="?/search" use:enhance class="search-form">
		<input
			type="search"
			name="q"
			placeholder={m.admin_blocks_search_placeholder()}
			aria-label={m.admin_blocks_search_aria()}
			autocomplete="off"
			minlength={MIN_SEARCH_LEN}
			bind:value={query}
		/>
		<button type="submit" class="btn">{m.admin_blocks_search_submit()}</button>
	</form>
	<p class="hint">{m.admin_blocks_search_hint()}</p>

	<label class="reason-field">
		<span>{m.admin_blocks_reason_label()}</span>
		<input type="text" placeholder={m.admin_blocks_reason_placeholder()} bind:value={reason} />
	</label>

	{#if searchError}
		<p class="inline-error" role="alert">{searchError}</p>
	{/if}

	<!-- Local results -->
	{#if localResults}
		{#if localResults.length === 0}
			<p class="empty" role="status">{m.admin_blocks_search_local_empty()}</p>
			<!-- ESI fallback opt-in. Uses the query the local search actually ran
			     (echoed back as f.query), falling back to the live input value. -->
			<form method="POST" action="?/esiSearch" use:enhance class="esi-cta-form">
				<input type="hidden" name="q" value={f?.query ?? query} />
				<button type="submit" class="btn ghost">{m.admin_blocks_search_esi_cta()}</button>
			</form>
		{:else}
			{@render resultList(localResults)}
		{/if}
	{/if}

	<!-- ESI results -->
	{#if esiResults}
		{#if esiUnavailable}
			<p class="notice" role="alert">{m.admin_blocks_esi_unavailable()}</p>
		{:else if esiResults.length === 0}
			<p class="empty" role="status">{m.admin_blocks_search_esi_empty()}</p>
		{:else}
			{@render resultList(esiResults)}
		{/if}
	{/if}
</section>

{#snippet resultList(results: (CharacterSearchResultDto | EsiCharacterSearchResultDto)[])}
	<ul class="results">
		{#each results as result (result.eve_character_id)}
			<li>
				<img class="portrait" src={result.portrait_url} alt="" width="32" height="32" />
				<span class="result-name">{result.name}</span>
				{#if result.already_blocked}
					<span class="blocked-badge">{m.admin_blocks_already_blocked()}</span>
				{:else}
					<!-- Selecting a result looks up its corp, then opens the confirm. -->
					<form method="POST" action="?/corpLookup" use:enhance class="select-form">
						<input type="hidden" name="eve_character_id" value={result.eve_character_id} />
						<button
							type="submit"
							class="btn select"
							onclick={() => chooseCharacter(result.eve_character_id, result.name)}
						>
							{m.admin_blocks_select()}
						</button>
					</form>
				{/if}
			</li>
		{/each}
	</ul>
{/snippet}

<!-- Hidden block form, submitted on confirm with the resolved id + reason. -->
<form bind:this={blockFormEl} method="POST" action="?/block" use:enhance class="hidden-form">
	<input bind:this={blockIdInput} type="hidden" name="eve_character_id" value="" />
	<input bind:this={blockReasonInput} type="hidden" name="reason" value="" />
</form>

<!-- Block confirmation (enriched with corp) -->
<ConfirmDialog
	open={confirmOpen}
	tone="danger"
	onCancel={() => {
		confirmOpen = false;
		selected = null;
	}}
	onConfirm={() => {
		if (selected && blockIdInput && blockReasonInput) {
			blockIdInput.value = String(selected.eve_character_id);
			blockReasonInput.value = reason;
			blockFormEl?.requestSubmit();
		}
		confirmOpen = false;
		selected = null;
		reason = '';
	}}
>
	{#snippet title()}{corpName
			? m.admin_blocks_confirm_title_corp({ name: selected?.name ?? '', corp: corpName })
			: m.admin_blocks_confirm_title({ name: selected?.name ?? '' })}{/snippet}
	{#snippet body()}{m.admin_blocks_confirm_body()}{/snippet}
	{#snippet confirmLabel()}{m.admin_blocks_confirm_submit()}{/snippet}
</ConfirmDialog>

<!-- Unblock confirmation -->
<ConfirmDialog
	open={unblockState.open}
	tone="danger"
	onCancel={() => (unblockState = { open: false, block: null })}
	onConfirm={() => {
		if (unblockState.block) {
			unblockForms[unblockState.block.eve_character_id]?.requestSubmit();
		}
		unblockState = { open: false, block: null };
	}}
>
	{#snippet title()}{m.admin_blocks_unblock_title({
			name: unblockState.block ? blockLabel(unblockState.block) : ''
		})}{/snippet}
	{#snippet body()}{m.admin_blocks_unblock_body()}{/snippet}
	{#snippet confirmLabel()}{m.admin_blocks_unblock_confirm()}{/snippet}
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
		margin: 0 0 12px;
		font-size: 0.75rem;
		color: var(--slate-400);
	}
	.hint {
		margin: 6px 0 0;
		font-size: 0.6875rem;
		color: var(--slate-500);
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

	.esi-cta-form {
		margin-top: 8px;
	}

	.results {
		list-style: none;
		margin: 12px 0 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.results li {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-800);
		border-radius: 4px;
	}
	.portrait {
		width: 32px;
		height: 32px;
		border-radius: 4px;
		flex-shrink: 0;
	}
	.result-name {
		flex: 1;
		font-size: 0.8125rem;
		color: var(--slate-100);
	}
	.blocked-badge {
		font-size: 0.625rem;
		color: var(--red);
		border: 1px solid var(--red);
		border-radius: 4px;
		padding: 1px 6px;
	}
	.select-form {
		margin: 0;
	}

	.notice {
		margin: 12px 0 0;
		padding: 8px 12px;
		background: rgba(245, 158, 11, 0.08);
		border: 1px solid var(--amber);
		border-radius: 4px;
		color: var(--amber);
		font-size: 0.75rem;
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
	.btn.ghost {
		color: var(--slate-400);
	}
	.btn.select {
		padding: 4px 12px;
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

	.reason-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		margin-top: 12px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.reason-field input {
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}

	.empty {
		padding: 16px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 12px 0 0;
	}

	.inline-error {
		margin: 8px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}
</style>
