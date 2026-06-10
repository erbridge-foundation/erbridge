<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type { AclSummaryDto } from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	type FormShape = { action: string; code?: string; message?: string; aclId?: string };
	let f = $derived(form as FormShape | null);

	let editError = $derived(f?.action === 'edit' && f.code ? f.message : null);
	let attachError = $derived(f?.action === 'attach' && f.code ? f.message : null);
	let detachError = $derived(
		f?.action === 'detach' && f.code ? { aclId: f.aclId, message: f.message } : null
	);

	let detachState = $state<{ open: boolean; acl: AclSummaryDto | null }>({
		open: false,
		acl: null
	});
	let detachForms = $state<Record<string, HTMLFormElement>>({});
</script>

<svelte:head>
	<title>{m.maps_settings_title({ name: data.map.name })}</title>
</svelte:head>

<main class="body">
	<div class="wrap">
		<a class="back" href="/maps/{data.map.slug}">{m.maps_settings_back()}</a>
		<h1 class="page-heading">{m.maps_settings_heading()}</h1>

		<section class="panel">
			<h2 class="panel-heading">{m.maps_settings_details_heading()}</h2>
			<form method="POST" action="?/edit" use:enhance class="edit-form">
				<input type="hidden" name="id" value={data.map.id} />
				<label>
					<span>{m.maps_field_name()}</span>
					<input name="name" value={data.map.name} required autocomplete="off" />
				</label>
				<label>
					<span>{m.maps_field_slug()}</span>
					<input name="slug" value={data.map.slug} required autocomplete="off" />
				</label>
				<label>
					<span>{m.maps_field_description()}</span>
					<textarea name="description" rows="3">{data.map.description ?? ''}</textarea>
				</label>
				<div class="form-actions">
					<button type="submit" class="btn primary">{m.maps_save()}</button>
				</div>
				{#if editError}
					<p class="inline-error" role="alert">{editError}</p>
				{/if}
			</form>
		</section>

		<section class="panel">
			<h2 class="panel-heading">{m.maps_detail_acls_heading()}</h2>
			{#if data.map.acls.length === 0}
				<p class="empty" role="status">{m.maps_detail_no_acls()}</p>
			{:else}
				<ul class="acl-list">
					{#each data.map.acls as acl (acl.id)}
						<li class="acl-row">
							<a class="acl-name" href="/acls/{acl.id}">{acl.name}</a>
							<form
								bind:this={detachForms[acl.id]}
								method="POST"
								action="?/detach"
								use:enhance
							>
								<input type="hidden" name="map_id" value={data.map.id} />
								<input type="hidden" name="acl_id" value={acl.id} />
								<button
									type="button"
									class="danger"
									onclick={() => (detachState = { open: true, acl })}
								>
									{m.maps_detail_detach()}
								</button>
							</form>
							{#if detachError?.aclId === acl.id}
								<p class="inline-error" role="alert">{detachError.message}</p>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}

			<div class="attach-block">
				<h3 class="sub-heading">{m.maps_detail_attach_heading()}</h3>
				{#if data.attachable.length === 0}
					<p class="empty" role="status">{m.maps_detail_no_attachable()}</p>
				{:else}
					<form method="POST" action="?/attach" use:enhance class="attach-form">
						<input type="hidden" name="map_id" value={data.map.id} />
						<select name="acl_id" aria-label={m.maps_detail_attach_label()} required>
							<option value="" disabled selected>{m.maps_detail_attach_placeholder()}</option>
							{#each data.attachable as acl (acl.id)}
								<option value={acl.id}>{acl.name}</option>
							{/each}
						</select>
						<button type="submit" class="btn">{m.maps_detail_attach()}</button>
					</form>
					{#if attachError}
						<p class="inline-error" role="alert">{attachError}</p>
					{/if}
				{/if}
			</div>
		</section>
	</div>
</main>

<ConfirmDialog
	open={detachState.open}
	tone="danger"
	onCancel={() => (detachState = { open: false, acl: null })}
	onConfirm={() => {
		if (detachState.acl) {
			detachForms[detachState.acl.id]?.requestSubmit();
		}
		detachState = { open: false, acl: null };
	}}
>
	{#snippet title()}{m.maps_detail_detach_title({ name: detachState.acl?.name ?? '' })}{/snippet}
	{#snippet body()}{m.maps_detail_detach_body()}{/snippet}
	{#snippet confirmLabel()}{m.maps_detail_detach_confirm()}{/snippet}
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

	.back {
		display: inline-block;
		margin-bottom: 12px;
		font-size: 0.75rem;
		color: var(--slate-400);
		text-decoration: none;
	}
	.back:hover {
		color: var(--slate-200);
	}

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
		margin: 0 0 12px;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	.sub-heading {
		margin: 0 0 8px;
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--slate-300);
	}

	.edit-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.edit-form label {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.edit-form input,
	.edit-form textarea {
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
		resize: vertical;
	}
	.edit-form input:focus,
	.edit-form textarea:focus {
		outline: none;
		border-color: var(--sky);
	}
	.form-actions {
		display: flex;
		justify-content: flex-end;
	}

	.acl-list {
		list-style: none;
		margin: 0 0 16px;
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
		padding: 10px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-800);
		border-radius: 4px;
	}
	.acl-name {
		font-size: 0.8125rem;
		color: var(--sky);
		text-decoration: none;
	}
	.acl-name:hover {
		text-decoration: underline;
	}

	.attach-block {
		margin-top: 16px;
		padding-top: 16px;
		border-top: 1px solid var(--space-800);
	}
	.attach-form {
		display: flex;
		gap: 8px;
	}
	.attach-form select {
		flex: 1;
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.attach-form select:focus {
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
