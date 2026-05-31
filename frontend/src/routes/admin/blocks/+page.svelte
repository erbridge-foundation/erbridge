<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type { BlockedCharacterDto } from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	function blockLabel(block: BlockedCharacterDto): string {
		return block.character_name ?? m.admin_blocks_unknown_name();
	}

	type FormError = {
		action: string;
		code: string;
		message: string;
		eveCharacterId?: number;
	};
	let formError = $derived(form && 'code' in form ? (form as unknown as FormError) : null);

	// Unblock confirmation state (one modal at a time).
	let unblockState = $state<{ open: boolean; block: BlockedCharacterDto | null }>({
		open: false,
		block: null
	});
	let unblockForms = $state<Record<number, HTMLFormElement>>({});
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
							{#if formError?.action === 'unblock' && formError?.eveCharacterId === block.eve_character_id}
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
	<h2 class="panel-heading">{m.admin_blocks_add_heading()}</h2>
	<p class="intro">{m.admin_blocks_add_intro()}</p>

	<form method="POST" action="?/block" use:enhance class="block-form">
		<label class="field">
			<span>{m.admin_blocks_id_label()}</span>
			<input
				type="number"
				name="eve_character_id"
				min="1"
				step="1"
				placeholder={m.admin_blocks_id_placeholder()}
				required
			/>
		</label>
		<label class="field">
			<span>{m.admin_blocks_reason_label()}</span>
			<input type="text" name="reason" placeholder={m.admin_blocks_reason_placeholder()} />
		</label>
		<button type="submit" class="btn">{m.admin_blocks_submit()}</button>
	</form>

	{#if formError?.action === 'block'}
		<p class="inline-error" role="alert" data-error-code={formError.code}>{formError.message}</p>
	{/if}
</section>

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

	.block-form {
		display: flex;
		align-items: flex-end;
		gap: 12px;
		flex-wrap: wrap;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.field input {
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.field input:focus {
		outline: none;
		border-color: var(--sky);
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
</style>
