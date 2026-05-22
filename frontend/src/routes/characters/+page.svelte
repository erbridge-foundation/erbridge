<script lang="ts">
	import { enhance } from '$app/forms';
	import type { PageData, ActionData } from './$types';

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
</script>

<main class="body">
	<div class="content">
		<div class="page-header">
			<h1>CHARACTERS</h1>
			<div class="header-actions">
				<label class="search">
					<svg
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
									<form method="POST" action="?/remove" use:enhance>
										<input type="hidden" name="character_id" value={character.id} />
										<button type="submit" class="danger">remove</button>
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
		<form method="POST" action="?/deleteAccount" use:enhance>
			<button type="submit" class="danger-btn">delete account</button>
		</form>
		{#if formError && !formError.characterId && formError.code}
			<p class="inline-error" role="alert" data-error-code={formError.code}>
				{formError.message}
			</p>
		{/if}
	</div>
</main>

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
		padding: 8px 12px 8px 32px;
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
	.search svg {
		position: absolute;
		left: 10px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--slate-500);
		pointer-events: none;
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
