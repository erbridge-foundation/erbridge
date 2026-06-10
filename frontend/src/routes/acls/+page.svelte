<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import type { AclDto } from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	type FormShape = { action: string; code?: string; message?: string; id?: string };
	let f = $derived(form as FormShape | null);

	let createOpen = $state(false);
	let createError = $derived(f?.action === 'create' && f.code ? f.message : null);
	let renameError = $derived(
		f?.action === 'rename' && f.code ? { id: f.id, message: f.message } : null
	);
	let deleteError = $derived(
		f?.action === 'delete' && f.code ? { id: f.id, message: f.message } : null
	);

	let editingId = $state<string | null>(null);

	let deleteState = $state<{ open: boolean; acl: AclDto | null }>({ open: false, acl: null });
	let deleteForms = $state<Record<string, HTMLFormElement>>({});
</script>

<svelte:head>
	<title>{m.acls_title()}</title>
</svelte:head>

<main class="body">
	<div class="wrap">
		<div class="head-row">
			<h1 class="page-heading">{m.acls_heading()}</h1>
			<button type="button" class="btn primary" onclick={() => (createOpen = true)}>
				{m.acls_create_open()}
			</button>
		</div>

		<section class="panel">
			{#if data.acls.length === 0}
				<p class="empty" role="status">{m.acls_empty()}</p>
			{:else}
				<ul class="acl-list">
					{#each data.acls as acl (acl.id)}
						<li class="acl-row">
							{#if editingId === acl.id}
								<form
									method="POST"
									action="?/rename"
									use:enhance={() => {
										return async ({ update }) => {
											await update({ reset: false });
											editingId = null;
										};
									}}
									class="rename-form"
								>
									<input type="hidden" name="id" value={acl.id} />
									<input name="name" value={acl.name} aria-label={m.acls_field_name()} required />
									<button type="submit" class="btn">{m.acls_save()}</button>
									<button type="button" class="btn ghost" onclick={() => (editingId = null)}>
										{m.dialog_cancel()}
									</button>
								</form>
							{:else}
								<a class="acl-name" href="/acls/{acl.id}">{acl.name}</a>
								<div class="acl-actions">
									<button type="button" class="btn ghost" onclick={() => (editingId = acl.id)}>
										{m.acls_rename()}
									</button>
									<form
										bind:this={deleteForms[acl.id]}
										method="POST"
										action="?/delete"
										use:enhance
									>
										<input type="hidden" name="id" value={acl.id} />
										<button
											type="button"
											class="danger"
											onclick={() => (deleteState = { open: true, acl })}
										>
											{m.acls_delete()}
										</button>
									</form>
								</div>
							{/if}
							{#if renameError?.id === acl.id}
								<p class="inline-error" role="alert">{renameError.message}</p>
							{/if}
							{#if deleteError?.id === acl.id}
								<p class="inline-error" role="alert">{deleteError.message}</p>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</section>

	</div>
</main>

<Modal open={createOpen} onClose={() => (createOpen = false)}>
	{#snippet title()}{m.acls_create_heading()}{/snippet}
	{#snippet children()}
		<form
			method="POST"
			action="?/create"
			use:enhance={() => {
				return async ({ update, result }) => {
					await update();
					if (result.type === 'success') createOpen = false;
				};
			}}
			class="create-form"
		>
			<label>
				<span>{m.acls_field_name()}</span>
				<input name="name" required autocomplete="off" />
			</label>
			{#if createError}
				<p class="inline-error" role="alert">{createError}</p>
			{/if}
			<div class="dialog-actions">
				<button type="button" class="btn ghost" onclick={() => (createOpen = false)}>
					{m.dialog_cancel()}
				</button>
				<button type="submit" class="btn primary">{m.acls_create_submit()}</button>
			</div>
		</form>
	{/snippet}
</Modal>

<ConfirmDialog
	open={deleteState.open}
	tone="danger"
	onCancel={() => (deleteState = { open: false, acl: null })}
	onConfirm={() => {
		if (deleteState.acl) {
			deleteForms[deleteState.acl.id]?.requestSubmit();
		}
		deleteState = { open: false, acl: null };
	}}
>
	{#snippet title()}{m.acls_delete_title({ name: deleteState.acl?.name ?? '' })}{/snippet}
	{#snippet body()}{m.acls_delete_body()}{/snippet}
	{#snippet confirmLabel()}{m.acls_delete_confirm()}{/snippet}
</ConfirmDialog>

<style>
	.body {
		flex: 1;
		overflow: auto;
		padding: 24px;
	}
	.wrap {
		max-width: 760px;
		margin: 0 auto;
	}

	.head-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}
	.page-heading {
		margin: 0;
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
	.acl-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.acl-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 12px;
		background: var(--space-950);
		border: 1px solid var(--space-800);
		border-radius: 4px;
	}
	.acl-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--sky);
		text-decoration: none;
	}
	.acl-name:hover {
		text-decoration: underline;
	}
	.acl-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.rename-form {
		display: flex;
		gap: 8px;
		width: 100%;
	}
	.rename-form input {
		flex: 1;
		padding: 6px 10px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}

	.create-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.create-form label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.create-form input {
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.create-form input:focus,
	.rename-form input:focus {
		outline: none;
		border-color: var(--sky);
	}

	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 12px;
		margin-top: 4px;
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
	.btn.primary {
		background: var(--sky);
		border-color: var(--sky);
		color: var(--space-950);
		font-weight: 600;
	}
	.btn.primary:hover {
		opacity: 0.9;
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
		flex-basis: 100%;
		margin: 4px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}
</style>
