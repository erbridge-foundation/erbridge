<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import type { CreatedKeyDto, KeyMetadataDto } from '$lib/api';
	import type { PageData, ActionData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	type Tab = 'api-keys' | 'danger-zone';
	let activeTab = $state<Tab>('api-keys');

	const tabs: ReadonlyArray<{ id: Tab; label: () => string }> = [
		{ id: 'api-keys', label: () => m.account_tab_api_keys() },
		{ id: 'danger-zone', label: () => m.account_tab_danger_zone() }
	];

	// --- create-form state ---------------------------------------------------
	let createOpen = $state(false);
	let createName = $state('');
	type ExpiryPreset = 'never' | '30d' | '90d' | '1y' | 'custom';
	let createExpiry = $state<ExpiryPreset>('never');
	let createCustomDate = $state('');
	let createNameError = $state(false);
	let computedExpiresAt = $state<string | null>(null);

	function resetCreateForm() {
		createName = '';
		createExpiry = 'never';
		createCustomDate = '';
		createNameError = false;
	}

	function computeExpiresAt(preset: ExpiryPreset, customDate: string): string | null {
		const now = Date.now();
		const day = 24 * 60 * 60 * 1000;
		switch (preset) {
			case 'never':
				return null;
			case '30d':
				return new Date(now + 30 * day).toISOString();
			case '90d':
				return new Date(now + 90 * day).toISOString();
			case '1y':
				return new Date(now + 365 * day).toISOString();
			case 'custom':
				// Interpret the date as end-of-day UTC (per design.md R3) so the user's
				// chosen date "expires that day" without timezone ambiguity.
				return customDate ? new Date(`${customDate}T23:59:59Z`).toISOString() : '';
		}
	}

	function onSubmitCreate(e: SubmitEvent) {
		if (createName.trim() === '') {
			e.preventDefault();
			createNameError = true;
			return;
		}
		// Compute the expires_at value right before submit so it goes into the
		// hidden field. Bound via the {computedExpiresAt} state below.
		computedExpiresAt = computeExpiresAt(createExpiry, createCustomDate);
	}

	// --- reveal panel state --------------------------------------------------
	// The plaintext is held in component-local $state, NOT just in `form`, so
	// the reveal panel survives an invalidateAll() that clears `form` (per
	// design.md D2). It is never persisted to storage.
	let revealedKey = $state<CreatedKeyDto | null>(null);
	let acknowledged = $state(false);
	let copyFailed = $state(false);
	let lastSeenCreateAt = $state<string | null>(null);

	$effect(() => {
		const created = (form as { createdKey?: CreatedKeyDto } | null)?.createdKey;
		if (created && created.created_at !== lastSeenCreateAt) {
			// Copy into local state FIRST, then refresh the list (R2: ordering).
			revealedKey = created;
			acknowledged = false;
			copyFailed = false;
			lastSeenCreateAt = created.created_at;
			createOpen = false;
			resetCreateForm();
			void invalidateAll();
		}
	});

	async function copyToClipboard() {
		if (!revealedKey) return;
		try {
			await navigator.clipboard.writeText(revealedKey.key);
			copyFailed = false;
		} catch {
			copyFailed = true;
		}
	}

	function dismissReveal() {
		revealedKey = null;
		acknowledged = false;
		copyFailed = false;
	}

	// --- list / formatting ---------------------------------------------------
	type KeyError = { code: string; message: string; keyId?: string };
	let formError = $derived(form as KeyError | null);

	function dateFormatter() {
		return new Intl.DateTimeFormat(getLocale(), {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	function formatDate(iso: string): string {
		return dateFormatter().format(new Date(iso));
	}

	function isExpired(expires_at: string | null): boolean {
		return expires_at !== null && new Date(expires_at).getTime() < Date.now();
	}

	let sortedKeys = $derived(
		[...data.keys].sort((a, b) => b.created_at.localeCompare(a.created_at))
	);

	// --- revoke flow ---------------------------------------------------------
	let revokeForms = $state<Record<string, HTMLFormElement>>({});
	let revokeState = $state<{ open: boolean; key: KeyMetadataDto | null }>({
		open: false,
		key: null
	});

	// --- delete-account flow -------------------------------------------------
	let deleteAccountOpen = $state(false);
	let deleteAccountForm = $state<HTMLFormElement | null>(null);
</script>

<svelte:head>
	<title>{m.account_title()}</title>
</svelte:head>

<main class="body">
	<div class="content">
		<div class="page-header">
			<h1>{m.account_heading()}</h1>
		</div>

		<div class="tabs" role="tablist" aria-label={m.account_heading()}>
			{#each tabs as tab (tab.id)}
				<button
					type="button"
					role="tab"
					id={`tab-${tab.id}`}
					aria-selected={activeTab === tab.id}
					aria-controls={`panel-${tab.id}`}
					class="tab"
					class:active={activeTab === tab.id}
					onclick={() => (activeTab = tab.id)}
				>
					{tab.label()}
				</button>
			{/each}
		</div>

		{#if activeTab === 'api-keys'}
			<div role="tabpanel" id="panel-api-keys" aria-labelledby="tab-api-keys">
				<p class="intro">{m.account_keys_intro()}</p>

				{#if revealedKey}
					<!-- Inline reveal panel (the only place plaintext ever renders). -->
					<section class="reveal" role="alert" aria-live="assertive" data-testid="reveal-panel">
						<h2 class="reveal-title">{m.account_keys_reveal_title()}</h2>
						<p class="reveal-warning">{m.account_keys_reveal_warning()}</p>

						<label class="reveal-key-label">
							{m.account_keys_reveal_key_label()}
							<div class="reveal-key-row">
								<code class="reveal-key" data-testid="reveal-key">{revealedKey.key}</code>
								<button
									type="button"
									class="copy-btn"
									data-testid="reveal-copy"
									onclick={copyToClipboard}
								>
									{m.account_keys_reveal_copy()}
								</button>
							</div>
						</label>
						{#if copyFailed}
							<p class="copy-failed" role="status">{m.account_keys_reveal_copy_failed()}</p>
						{/if}

						<label class="ack">
							<input
								type="checkbox"
								bind:checked={acknowledged}
								data-testid="reveal-ack"
							/>
							{m.account_keys_reveal_ack()}
						</label>

						<div class="reveal-actions">
							<button
								type="button"
								class="done-btn"
								disabled={!acknowledged}
								onclick={dismissReveal}
								data-testid="reveal-done"
							>
								{m.account_keys_reveal_done()}
							</button>
						</div>
					</section>
				{/if}

				{#if sortedKeys.length === 0 && !createOpen}
					<div class="empty-state">
						<p class="empty-title">{m.account_keys_empty_title()}</p>
						<button type="button" class="primary" onclick={() => (createOpen = true)}>
							{m.account_keys_empty_cta()}
						</button>
					</div>
				{:else}
					{#if !createOpen}
						<div class="list-actions">
							<button type="button" class="primary" onclick={() => (createOpen = true)}>
								{m.account_keys_new()}
							</button>
						</div>
					{/if}

					{#if sortedKeys.length > 0}
						<table class="keys-table">
							<thead>
								<tr>
									<th>{m.account_keys_col_name()}</th>
									<th>{m.account_keys_col_created()}</th>
									<th>{m.account_keys_col_expires()}</th>
									<th class="actions-col">{m.account_keys_col_actions()}</th>
								</tr>
							</thead>
							<tbody>
								{#each sortedKeys as key (key.id)}
									{@const expired = isExpired(key.expires_at)}
									<tr class:expired data-key-id={key.id}>
										<td>{key.name}</td>
										<td>{formatDate(key.created_at)}</td>
										<td>
											{#if key.expires_at === null}
												{m.account_keys_expires_never()}
											{:else if expired}
												<span class="badge-expired">{m.account_keys_badge_expired()}</span>
											{:else}
												{formatDate(key.expires_at)}
											{/if}
										</td>
										<td class="actions-col">
											<form
												bind:this={revokeForms[key.id]}
												method="POST"
												action="?/revokeKey"
												use:enhance
											>
												<input type="hidden" name="key_id" value={key.id} />
												<!-- type="button": no-JS users get no confirmation, matching /characters' v1 regression. -->
												<button
													type="button"
													class="danger"
													onclick={() => {
														revokeState = { open: true, key };
													}}
												>
													{m.account_keys_revoke()}
												</button>
											</form>
											{#if formError?.keyId === key.id && formError?.code}
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
				{/if}

				{#if createOpen}
					<section class="create-form-wrapper">
						<form
							method="POST"
							action="?/createKey"
							use:enhance
							onsubmit={onSubmitCreate}
							class="create-form"
						>
							<label class="field">
								<span>{m.account_keys_create_name_label()}</span>
								<input
									type="text"
									name="name"
									bind:value={createName}
									placeholder={m.account_keys_create_name_placeholder()}
									autocomplete="off"
									aria-invalid={createNameError}
								/>
							</label>
							{#if createNameError}
								<p class="inline-error" role="alert">{m.account_keys_create_name_required()}</p>
							{/if}

							<fieldset class="expiry">
								<legend>{m.account_keys_create_expiry_label()}</legend>
								<label class="radio">
									<input
										type="radio"
										bind:group={createExpiry}
										value={'never'}
									/>
									{m.account_keys_create_expiry_never()}
								</label>
								<label class="radio">
									<input type="radio" bind:group={createExpiry} value={'30d'} />
									{m.account_keys_create_expiry_30d()}
								</label>
								<label class="radio">
									<input type="radio" bind:group={createExpiry} value={'90d'} />
									{m.account_keys_create_expiry_90d()}
								</label>
								<label class="radio">
									<input type="radio" bind:group={createExpiry} value={'1y'} />
									{m.account_keys_create_expiry_1y()}
								</label>
								<label class="radio">
									<input type="radio" bind:group={createExpiry} value={'custom'} />
									{m.account_keys_create_expiry_custom()}
								</label>
								{#if createExpiry === 'custom'}
									<label class="field custom-date">
										<span>{m.account_keys_create_custom_label()}</span>
										<input type="date" bind:value={createCustomDate} />
									</label>
								{/if}
							</fieldset>

							<input
								type="hidden"
								name="expires_at"
								value={computedExpiresAt ?? ''}
							/>

							<div class="create-actions">
								<button type="submit" class="primary">
									{m.account_keys_create_submit()}
								</button>
								<button
									type="button"
									class="ghost"
									onclick={() => {
										createOpen = false;
										resetCreateForm();
									}}
								>
									{m.account_keys_create_cancel()}
								</button>
							</div>

							{#if formError && !formError.keyId && formError.code}
								<p class="inline-error" role="alert" data-error-code={formError.code}>
									{formError.message}
								</p>
							{/if}
						</form>
					</section>
				{/if}
			</div>
		{:else}
			<div role="tabpanel" id="panel-danger-zone" aria-labelledby="tab-danger-zone">
				<h2 class="danger-zone-title">{m.account_danger_zone()}</h2>
				<form
					bind:this={deleteAccountForm}
					method="POST"
					action="?/deleteAccount"
					use:enhance
				>
					<!-- type="button": no-JS users get no confirmation, matching /characters' v1 regression. -->
					<button
						type="button"
						class="danger-btn"
						onclick={() => (deleteAccountOpen = true)}
					>
						{m.account_delete_account()}
					</button>
				</form>
				{#if formError && !formError.keyId && formError.code}
					<p class="inline-error" role="alert" data-error-code={formError.code}>
						{formError.message}
					</p>
				{/if}
			</div>
		{/if}
	</div>
</main>

<!-- Per-row revoke confirmation modal. -->
<ConfirmDialog
	open={revokeState.open}
	tone="danger"
	onCancel={() => (revokeState = { open: false, key: null })}
	onConfirm={() => {
		if (revokeState.key) {
			revokeForms[revokeState.key.id]?.requestSubmit();
		}
		revokeState = { open: false, key: null };
	}}
>
	{#snippet title()}{m.account_keys_revoke_title({ name: revokeState.key?.name ?? '' })}{/snippet}
	{#snippet body()}{m.account_keys_revoke_body()}{/snippet}
	{#snippet confirmLabel()}{m.account_keys_revoke_confirm()}{/snippet}
</ConfirmDialog>

<!-- Delete-account confirmation modal (lifted from /characters). -->
<ConfirmDialog
	open={deleteAccountOpen}
	tone="danger"
	onCancel={() => (deleteAccountOpen = false)}
	onConfirm={() => {
		deleteAccountForm?.requestSubmit();
		deleteAccountOpen = false;
	}}
>
	{#snippet title()}{m.account_delete_account_title()}{/snippet}
	{#snippet body()}{m.account_delete_account_body()}{/snippet}
	{#snippet confirmLabel()}{m.account_delete_account_confirm()}{/snippet}
</ConfirmDialog>

<style>
	.body {
		flex: 1;
		overflow: auto;
		display: flex;
		justify-content: center;
		padding: 32px 24px 48px;
	}
	.content {
		width: 100%;
		max-width: 960px;
	}

	.page-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		margin-bottom: 16px;
		flex-wrap: wrap;
	}
	.page-header h1 {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-500);
	}

	.tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 16px;
		border-bottom: 1px solid var(--space-700);
	}
	.tab {
		padding: 8px 14px;
		font: inherit;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--slate-400);
		background: transparent;
		border: 0;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		cursor: pointer;
	}
	.tab:hover {
		color: var(--slate-100);
	}
	.tab.active {
		color: var(--sky);
		border-bottom-color: var(--sky);
	}
	.tab:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	.intro {
		margin: 0 0 16px;
		font-size: 0.8125rem;
		line-height: 1.55;
		color: var(--slate-400);
	}

	.list-actions {
		display: flex;
		justify-content: flex-end;
		margin-bottom: 12px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 16px;
		padding: 48px 24px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		text-align: center;
	}
	.empty-title {
		margin: 0;
		font-size: 0.875rem;
		color: var(--slate-300);
	}

	.primary {
		display: inline-flex;
		align-items: center;
		padding: 8px 16px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.primary:hover {
		background: var(--space-700);
	}

	.ghost {
		background: transparent;
		border: 0;
		padding: 8px 12px;
		color: var(--slate-400);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.ghost:hover {
		color: var(--slate-100);
	}

	.keys-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.8125rem;
	}
	.keys-table th,
	.keys-table td {
		text-align: left;
		padding: 10px 12px;
		border-bottom: 1px solid var(--space-700);
	}
	.keys-table th {
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: var(--slate-500);
	}
	.keys-table td {
		color: var(--slate-200);
	}
	.keys-table tr.expired td {
		color: var(--slate-500);
	}
	.actions-col {
		text-align: right;
	}

	.badge-expired {
		display: inline-flex;
		align-items: center;
		padding: 1px 6px;
		border-radius: 4px;
		background: rgba(239, 68, 68, 0.1);
		border: 1px solid rgba(239, 68, 68, 0.35);
		color: var(--red);
		font-size: 0.625rem;
		letter-spacing: 0.05em;
		text-transform: uppercase;
	}

	.danger {
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: 0.8125rem;
		color: var(--slate-400);
		cursor: pointer;
	}
	.danger:hover {
		color: var(--red);
	}

	.inline-error {
		margin: 6px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}

	.create-form-wrapper {
		margin-top: 16px;
		padding: 16px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
	}
	.create-form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--slate-300);
	}
	.field input[type='text'],
	.field input[type='date'] {
		padding: 8px 10px;
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
	.field input[aria-invalid='true'] {
		border-color: var(--red);
	}

	.expiry {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		padding: 8px 0;
		border: 0;
		margin: 0;
	}
	.expiry legend {
		font-size: 0.75rem;
		color: var(--slate-300);
		padding-bottom: 4px;
		width: 100%;
	}
	.radio {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 0.8125rem;
		color: var(--slate-200);
		cursor: pointer;
	}
	.custom-date {
		flex-basis: 100%;
	}

	.create-actions {
		display: flex;
		gap: 12px;
		margin-top: 4px;
	}

	.reveal {
		margin: 16px 0 24px;
		padding: 16px;
		background: rgba(56, 189, 248, 0.05);
		border: 1px solid var(--sky);
		border-radius: 6px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.reveal-title {
		margin: 0;
		font-size: 0.9375rem;
		color: var(--sky);
	}
	.reveal-warning {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--amber);
	}
	.reveal-key-label {
		display: flex;
		flex-direction: column;
		gap: 6px;
		font-size: 0.75rem;
		color: var(--slate-300);
	}
	.reveal-key-row {
		display: flex;
		align-items: stretch;
		gap: 8px;
	}
	.reveal-key {
		flex: 1;
		padding: 10px 12px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-size: 0.8125rem;
		color: var(--slate-100);
		word-break: break-all;
		user-select: all;
	}
	.copy-btn {
		padding: 8px 14px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.copy-btn:hover {
		background: var(--space-700);
	}
	.copy-failed {
		margin: 0;
		font-size: 0.75rem;
		color: var(--amber);
	}
	.ack {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		font-size: 0.8125rem;
		color: var(--slate-200);
		cursor: pointer;
	}
	.reveal-actions {
		display: flex;
		justify-content: flex-end;
	}
	.done-btn {
		padding: 8px 16px;
		background: transparent;
		border: 1px solid var(--sky);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.done-btn:hover:not(:disabled) {
		background: var(--sky);
		color: var(--space-950);
	}
	.done-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.danger-zone-title {
		margin: 8px 0 16px;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-500);
	}
	.danger-btn {
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: 0.875rem;
		color: var(--red);
		cursor: pointer;
	}
	.danger-btn:hover {
		text-decoration: underline;
	}
</style>
