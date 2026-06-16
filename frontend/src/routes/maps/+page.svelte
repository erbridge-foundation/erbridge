<script lang="ts">
	import { enhance } from "$app/forms";
	import { m } from "$lib/paraglide/messages";
	import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
	import Modal from "$lib/components/Modal.svelte";
	import type { MapDto } from "$lib/api";
	import type { PageData, ActionData } from "./$types";

	let { data, form }: { data: PageData; form: ActionData } = $props();

	type FormShape = {
		action: string;
		code?: string;
		message?: string;
		id?: string;
	};
	let f = $derived(form as FormShape | null);

	let createError = $derived(
		f?.action === "create" && f.code ? f.message : null,
	);
	let deleteError = $derived(
		f?.action === "delete" && f.code ? { id: f.id, message: f.message } : null,
	);

	// Create dialog.
	let createOpen = $state(false);

	// Delete confirmation (one at a time).
	let deleteState = $state<{ open: boolean; map: MapDto | null }>({
		open: false,
		map: null,
	});
	let deleteForms = $state<Record<string, HTMLFormElement>>({});
</script>

<svelte:head>
	<title>{m.maps_title()}</title>
</svelte:head>

<main class="body">
	<div class="wrap">
		<div class="head-row">
			<h1 class="page-heading">{m.maps_heading()}</h1>
			<button
				type="button"
				class="btn primary"
				onclick={() => (createOpen = true)}
			>
				{m.maps_create_open()}
			</button>
		</div>

		<!-- Temporary: link to the disposable map-canvas prototype. Removed when the
		     real /maps/[slug] canvas lands (see build-map-canvas-prototype). -->
		<a class="proto-link" href="/maps/_proto">
			<span class="proto-link-label">{m.map_proto_link()}</span>
			<span class="proto-link-note">{m.map_proto_link_note()}</span>
		</a>

		<section class="panel">
			{#if data.maps.length === 0}
				<p class="empty" role="status">{m.maps_empty()}</p>
			{:else}
				<ul class="map-list">
					{#each data.maps as map (map.id)}
						<li class="map-row">
							<div class="map-main">
								<a class="map-name" href="/maps/{map.slug}">{map.name}</a>
								{#if map.description}<span class="map-desc"
										>{map.description}</span
									>{/if}
								<!-- ACL summary is shown only when the viewer can see attached ACLs.
								     An empty list does not mean "no ACLs" (the viewer may simply
								     lack manage permission on them), so no "none" text is shown. -->
								{#if map.acls.length > 0}
									<span class="map-acls">
										{m.maps_acls_label()}: {map.acls
											.map((a) => a.name)
											.join(", ")}
									</span>
								{/if}
							</div>
							<div class="map-actions">
								<a class="btn ghost" href="/maps/{map.slug}/settings"
									>{m.maps_edit()}</a
								>
								<form
									bind:this={deleteForms[map.id]}
									method="POST"
									action="?/delete"
									use:enhance
								>
									<input type="hidden" name="id" value={map.id} />
									<button
										type="button"
										class="danger"
										onclick={() => (deleteState = { open: true, map })}
									>
										{m.maps_delete()}
									</button>
								</form>
							</div>
							{#if deleteError?.id === map.id}
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
	{#snippet title()}{m.maps_create_heading()}{/snippet}
	{#snippet children()}
		<form
			method="POST"
			action="?/create"
			use:enhance={() => {
				return async ({ update, result }) => {
					await update();
					if (result.type === "success") createOpen = false;
				};
			}}
			class="create-form"
		>
			<label>
				<span>{m.maps_field_name()}</span>
				<input name="name" required autocomplete="off" />
			</label>
			<label>
				<span>{m.maps_field_slug()}</span>
				<input name="slug" required autocomplete="off" />
			</label>
			<label>
				<span>{m.maps_field_description()}</span>
				<textarea name="description" rows="3"></textarea>
			</label>
			<label class="checkbox">
				<input type="checkbox" name="default_acl" />
				<span>
					<span class="checkbox-label">{m.maps_create_default_acl()}</span>
					<span class="checkbox-hint">{m.maps_create_default_acl_hint()}</span>
				</span>
			</label>
			{#if createError}
				<p class="inline-error" role="alert">{createError}</p>
			{/if}
			<div class="dialog-actions">
				<button
					type="button"
					class="btn ghost"
					onclick={() => (createOpen = false)}
				>
					{m.dialog_cancel()}
				</button>
				<button type="submit" class="btn primary"
					>{m.maps_create_submit()}</button
				>
			</div>
		</form>
	{/snippet}
</Modal>

<ConfirmDialog
	open={deleteState.open}
	tone="danger"
	onCancel={() => (deleteState = { open: false, map: null })}
	onConfirm={() => {
		if (deleteState.map) {
			deleteForms[deleteState.map.id]?.requestSubmit();
		}
		deleteState = { open: false, map: null };
	}}
>
	{#snippet title()}{m.maps_delete_title({
			name: deleteState.map?.name ?? "",
		})}{/snippet}
	{#snippet body()}{m.maps_delete_body()}{/snippet}
	{#snippet confirmLabel()}{m.maps_delete_confirm()}{/snippet}
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

	/* Temporary prototype link (see markup note). */
	.proto-link {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-bottom: 16px;
		padding: 12px 16px;
		background: var(--space-900);
		border: 1px dashed var(--violet);
		border-radius: 6px;
		text-decoration: none;
	}
	.proto-link:hover {
		background: var(--space-800);
	}
	.proto-link:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.proto-link-label {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--violet);
	}
	.proto-link-note {
		font-size: 0.6875rem;
		color: var(--slate-500);
	}

	.panel {
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		padding: 20px;
	}

	.map-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.map-row {
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
	.map-main {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 8px;
		min-width: 0;
	}
	.map-name {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--sky);
		text-decoration: none;
	}
	.map-name:hover {
		text-decoration: underline;
	}
	.map-desc {
		font-size: 0.75rem;
		color: var(--slate-400);
	}
	.map-acls {
		flex-basis: 100%;
		font-size: 0.6875rem;
		color: var(--slate-500);
	}
	.map-actions {
		display: flex;
		align-items: center;
		gap: 8px;
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
	.create-form input:not([type="checkbox"]),
	.create-form textarea {
		padding: 8px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
		resize: vertical;
	}
	.create-form input:focus,
	.create-form textarea:focus {
		outline: none;
		border-color: var(--sky);
	}

	.checkbox {
		flex-direction: row !important;
		align-items: flex-start;
		gap: 8px;
	}
	.checkbox input {
		margin-top: 2px;
		accent-color: var(--sky);
		cursor: pointer;
	}
	.checkbox input:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.checkbox-label {
		display: block;
		font-size: 0.8125rem;
		color: var(--slate-200);
	}
	.checkbox-hint {
		display: block;
		margin-top: 2px;
		font-size: 0.6875rem;
		color: var(--slate-500);
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
		text-decoration: none;
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
		background: var(--sky);
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
