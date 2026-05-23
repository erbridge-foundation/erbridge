<script lang="ts">
	import { enhance } from '$app/forms';
	import type { PageData, ActionData } from './$types';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	let query = $state('');

	let filtered = $derived(
		query.trim() === ''
			? data.characters
			: data.characters.filter((c) =>
					c.name.toLowerCase().includes(query.trim().toLowerCase())
				)
	);

	// The form result's characterId is present on setMain/remove failures but
	// not on deleteAccount; widen the type so the template can branch on it.
	type FormError = { code: string; message: string; characterId?: string };
	let formError = $derived(form as FormError | null);

	// Per-character remove confirmation state. A single pair covers all cards;
	// only one modal is open at a time.
	type Character = (typeof data.characters)[0];
	let removeState = $state<{ open: boolean; character: Character | null }>({
		open: false,
		character: null
	});

	// Map of character id → form element, populated via bind:this in the template.
	let removeForms = $state<Record<string, HTMLFormElement>>({});

	// Delete-account confirmation state.
	let deleteAccountOpen = $state(false);
	let deleteAccountForm = $state<HTMLFormElement | null>(null);
</script>

<svelte:head>
	<title>E-R Bridge — Characters</title>
</svelte:head>

<main class="body">
	<div class="content">
		<div class="page-header">
			<h1>CHARACTERS</h1>
			<div class="header-actions">
				<!-- TODO: extract a SearchInput component on the second use. -->
				<label class="search">
					<svg
						class="search-icon"
						width="14"
						height="14"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						aria-hidden="true"
					>
						<circle cx="11" cy="11" r="7"></circle>
						<line x1="21" y1="21" x2="16.65" y2="16.65"></line>
					</svg>
					<input
						type="search"
						placeholder="search characters…"
						aria-label="Search characters by name"
						autocomplete="off"
						bind:value={query}
					/>
					{#if query !== ''}
						<button
							type="button"
							class="search-clear"
							aria-label="Clear search"
							onclick={() => (query = '')}
						>
							<svg
								width="12"
								height="12"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2.5"
								aria-hidden="true"
							>
								<line x1="6" y1="6" x2="18" y2="18"></line>
								<line x1="18" y1="6" x2="6" y2="18"></line>
							</svg>
						</button>
					{/if}
				</label>
				<a class="btn" href="/auth/characters/add?return_to=/characters">+ add character</a>
			</div>
		</div>

		<div class="grid" class:empty={filtered.length === 0}>
			{#if filtered.length === 0}
				<p class="empty-state" role="status">No characters match your search.</p>
			{/if}

			{#each filtered as character (character.id)}
				<div class="card-wrapper">
					<article class="card" data-name={character.name}>
						<div class="card-top">
							<img
								class="portrait"
								src={character.portrait_url}
								alt=""
								width="56"
								height="56"
							/>
							<div class="info">
								<div class="name-row">
									<span class="name">{character.name}</span>
									{#if character.is_main}
										<span class="badge-main">main</span>
									{/if}
								</div>
								<div class="affiliation">
									<span class="label">{character.corporation_name}</span>
								</div>
								{#if character.alliance_name}
									<div class="affiliation alliance">
										<span class="label">{character.alliance_name}</span>
									</div>
								{/if}
							</div>
						</div>

						<div class="card-footer">
							<span
								class="token-status"
								data-state={character.token_status}
							>
								<span class="dot" aria-hidden="true"></span>
								<span>token {character.token_status === 'active' ? 'active' : 'expired'}</span>
							</span>

							{#if !character.is_main}
								<div class="actions">
									{#if character.token_status === 'active'}
										<form method="POST" action="?/setMain" use:enhance>
											<input type="hidden" name="character_id" value={character.id} />
											<button type="submit">set main</button>
										</form>
									{:else}
										<a class="reauth" href="/auth/characters/add?return_to=/characters">re-auth</a>
									{/if}
									<form
										bind:this={removeForms[character.id]}
										method="POST"
										action="?/remove"
										use:enhance
									>
										<input type="hidden" name="character_id" value={character.id} />
										<!-- type="button": no-JS users get no confirmation and no submit (§3.5).
										     Per design.md decision 8, this regression is accepted for v1. -->
										<button
											type="button"
											class="danger"
											onclick={() => {
												removeState = { open: true, character };
											}}
										>remove</button>
									</form>
								</div>
							{:else if character.token_status === 'expired'}
								<div class="actions">
									<a class="reauth" href="/auth/characters/add?return_to=/characters">re-auth</a>
								</div>
							{/if}
						</div>
					</article>

					{#if formError?.characterId === character.id && formError?.code}
						<p class="inline-error" role="alert" data-error-code={formError.code}>
							{formError.message}
						</p>
					{/if}
				</div>
			{/each}
		</div>

		<hr class="divider" />

		<h2 class="danger-zone-title">DANGER ZONE</h2>
		<form bind:this={deleteAccountForm} method="POST" action="?/deleteAccount" use:enhance>
			<!-- type="button": no-JS users get no confirmation and no submit (§3.5).
			     Per design.md decision 8, this regression is accepted for v1. -->
			<button type="button" class="danger-btn" onclick={() => (deleteAccountOpen = true)}>
				delete account
			</button>
		</form>
		{#if formError && !formError.characterId && formError.code}
			<p class="inline-error" role="alert" data-error-code={formError.code}>
				{formError.message}
			</p>
		{/if}
	</div>
</main>

<!-- Per-character remove confirmation modal (§3.2). -->
<ConfirmDialog
	open={removeState.open}
	tone="danger"
	onCancel={() => (removeState = { open: false, character: null })}
	onConfirm={() => {
		if (removeState.character) {
			removeForms[removeState.character.id]?.requestSubmit();
		}
		removeState = { open: false, character: null };
	}}
>
	{#snippet title()}Remove {removeState.character?.name}?{/snippet}
	{#snippet body()}
		This character will be removed from your account. You can add them again at any time
		via add character and performing an EVE login.
	{/snippet}
	{#snippet confirmLabel()}remove character{/snippet}
</ConfirmDialog>

<!-- Delete-account confirmation modal (§3.3). -->
<ConfirmDialog
	open={deleteAccountOpen}
	tone="danger"
	onCancel={() => (deleteAccountOpen = false)}
	onConfirm={() => {
		deleteAccountForm?.requestSubmit();
		deleteAccountOpen = false;
	}}
>
	{#snippet title()}Delete account?{/snippet}
	{#snippet body()}
		Your account will be deactivated. To restore it, log back in within 30 days; after
		that, your data is permanently removed.
	{/snippet}
	{#snippet confirmLabel()}delete account{/snippet}
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

	.header-actions {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.search {
		position: relative;
		width: 260px;
	}
	.search input {
		width: 100%;
		padding: 8px 32px 8px 32px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.75rem;
	}
	.search input::placeholder {
		color: var(--slate-500);
	}
	.search input:focus {
		outline: none;
		border-color: var(--sky);
	}

	/* Hide the browser-native cancel button on type=search so our custom X
	   is the only one shown. */
	.search input::-webkit-search-cancel-button,
	.search input::-webkit-search-decoration {
		appearance: none;
		-webkit-appearance: none;
	}

	.search-icon {
		position: absolute;
		left: 10px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--slate-500);
		pointer-events: none;
	}

	.search-clear {
		position: absolute;
		right: 6px;
		top: 50%;
		transform: translateY(-50%);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		padding: 0;
		background: transparent;
		border: 0;
		border-radius: 3px;
		color: var(--slate-500);
		cursor: pointer;
	}
	.search-clear:hover {
		color: var(--slate-100);
		background: var(--space-700);
	}
	.search-clear:focus-visible {
		outline: 1px solid var(--sky);
		outline-offset: 1px;
	}

	.btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 8px 16px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		text-decoration: none;
		cursor: pointer;
		white-space: nowrap;
	}
	.btn:hover {
		background: var(--space-700);
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 16px;
	}
	@media (max-width: 600px) {
		.grid {
			grid-template-columns: 1fr;
		}
	}

	.empty-state {
		grid-column: 1 / -1;
		padding: 32px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}

	.card-wrapper {
		display: flex;
		flex-direction: column;
	}

	.card {
		display: flex;
		flex-direction: column;
		gap: 12px;
		padding: 16px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		height: 100%;
	}


	.card-top {
		display: flex;
		gap: 12px;
		align-items: flex-start;
	}

	.portrait {
		width: 56px;
		height: 56px;
		border-radius: 4px;
		flex-shrink: 0;
		display: block;
	}

	.info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.name-row {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	.name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.badge-main {
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
		flex-shrink: 0;
	}

	.affiliation {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.75rem;
		color: var(--slate-300);
		min-width: 0;
	}
	.affiliation .label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.affiliation.alliance {
		color: var(--slate-400);
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

	.card-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding-top: 8px;
		border-top: 1px solid var(--space-700);
		/* Push the footer to the bottom so cards in a row line up even when
		   their content (alliance row, etc.) differs in height. */
		margin-top: auto;
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 16px;
		color: var(--slate-300);
		font-size: 0.75rem;
	}
	.actions form {
		margin: 0;
		padding: 0;
	}
	.actions button {
		background: transparent;
		border: 0;
		padding: 0;
		font: inherit;
		color: inherit;
		cursor: pointer;
	}
	.actions button:hover {
		color: var(--slate-100);
	}
	.actions button.danger {
		color: var(--slate-400);
	}
	.actions button.danger:hover {
		color: var(--red);
	}
	.actions .reauth {
		color: var(--amber);
		text-decoration: none;
	}
	.actions .reauth:hover {
		color: var(--slate-100);
	}

	.inline-error {
		margin: 8px 0 0;
		color: var(--red);
		font-size: 0.75rem;
	}

	.divider {
		margin: 32px 0 24px;
		border: 0;
		border-top: 1px solid var(--space-700);
	}

	.danger-zone-title {
		margin: 0 0 16px;
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
