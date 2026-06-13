<!--
	MemberPicker.svelte — entity-search-backed picker for ACL members.

	Mirrors the block-picker's search→act flow (admin/blocks). The parent owns the
	`search` and `addMember` form actions; this component renders the search box
	and grouped results, and each result row is itself an inline add form carrying
	the already-resolved identifier the add-member action needs:

	  character            → eve_entity_id (the EVE character id) always, plus
	                         character_id (the eve_character.id UUID) ONLY when a
	                         local row already exists (c.id != null). When unknown,
	                         the add omits character_id and the backend mints the
	                         orphan from eve_entity_id.
	  corporation/alliance → eve_entity_id

	Each result has its own permission `<select>` gated to the member type
	(manage/admin character-only) and an inline "add" button, so there is no
	separate "select then scroll to a role box" step.

	The 3-char minimum is enforced here (no request fires below it) AND by the
	parent action. While a search is in flight the input shows a "searching"
	indicator. The "unavailable" outcome renders a distinct state, never conflated
	with "no matches".
-->
<script lang="ts">
	import { enhance } from '$app/forms';
	import { m } from '$lib/paraglide/messages';
	import { permissionsFor, type MemberType, type Permission } from '$lib/acl-permissions';
	import type { EntityCharacterDto, EntityOrgDto } from '$lib/api';

	let {
		characters = [],
		corporations = [],
		alliances = [],
		unavailable = false,
		searched = false,
		errorMessage = null
	}: {
		characters?: EntityCharacterDto[];
		corporations?: EntityOrgDto[];
		alliances?: EntityOrgDto[];
		unavailable?: boolean;
		searched?: boolean;
		errorMessage?: string | null;
	} = $props();

	const MIN_SEARCH_LEN = 3;

	// EVE public image CDN — portraits/logos are derivable from the entity id, so
	// the search endpoint doesn't need to return them. Helps avoid picking the
	// wrong same-named character/corp/alliance.
	const IMG_BASE = 'https://images.evetech.net';
	const characterPortrait = (eveCharacterId: number) =>
		`${IMG_BASE}/characters/${eveCharacterId}/portrait?size=64`;
	const corporationLogo = (eveEntityId: number) =>
		`${IMG_BASE}/corporations/${eveEntityId}/logo?size=64`;
	const allianceLogo = (eveEntityId: number) => `${IMG_BASE}/alliances/${eveEntityId}/logo?size=64`;

	let query = $state('');
	let tooShort = $derived(query.trim().length > 0 && query.trim().length < MIN_SEARCH_LEN);

	// Scope narrows the ESI search to a single category so the call is quicker.
	// 'any' searches all three (the action omits `categories`, the backend default).
	type Scope = 'character' | 'corporation' | 'alliance' | 'any';
	let scope = $state<Scope>('any');
	const SCOPES: Scope[] = ['character', 'corporation', 'alliance', 'any'];
	function scopeLabel(s: Scope): string {
		switch (s) {
			case 'character':
				return m.picker_scope_character();
			case 'corporation':
				return m.picker_scope_corporation();
			case 'alliance':
				return m.picker_scope_alliance();
			case 'any':
				return m.picker_scope_any();
		}
	}

	// True while the search request is in flight (set by use:enhance), drives the
	// active-search visual indicator.
	let searching = $state(false);

	let hasResults = $derived(
		characters.length > 0 || corporations.length > 0 || alliances.length > 0
	);

	// Per-result permission selection, keyed by a stable id. Defaults to 'read'.
	let perms = $state<Record<string, Permission>>({});
	function permFor(key: string): Permission {
		return perms[key] ?? 'read';
	}

	// Reset the per-result selections whenever a new search's results arrive, so a
	// permission picked for a row in one search doesn't leak onto a same-keyed row
	// in an unrelated later search. The identity signal is the concatenation of the
	// current result ids; when it changes, the result set changed.
	let resultsKey = $derived(
		[
			// Key on the always-present eve_character_id, not the now-nullable c.id.
			...characters.map((c) => `char-${c.eve_character_id}`),
			...corporations.map((o) => `corp-${o.eve_entity_id}`),
			...alliances.map((o) => `ally-${o.eve_entity_id}`)
		].join('|')
	);
	$effect(() => {
		// Touch resultsKey so this effect re-runs when the result set changes.
		resultsKey;
		perms = {};
	});

	function permLabel(p: Permission): string {
		switch (p) {
			case 'read':
				return m.acl_perm_read();
			case 'read_write':
				return m.acl_perm_read_write();
			case 'manage':
				return m.acl_perm_manage();
			case 'admin':
				return m.acl_perm_admin();
			case 'deny':
				return m.acl_perm_deny();
		}
	}
</script>

<div class="member-picker">
	<!-- The 3-char guard disables submit below the minimum (matching the action's
	     server-side too_short guard). A <form> submits on Enter natively, so the
	     user can search by pressing Enter in the input. -->
	<form
		method="POST"
		action="?/search"
		use:enhance={() => {
			searching = true;
			return async ({ update }) => {
				await update({ reset: false });
				searching = false;
			};
		}}
		class="search-form"
		class:searching
	>
		<div class="search-row">
			<div class="search-field">
				<input
					type="search"
					name="q"
					placeholder={m.picker_search_placeholder()}
					aria-label={m.picker_search_aria()}
					autocomplete="off"
					minlength={MIN_SEARCH_LEN}
					bind:value={query}
				/>
				{#if searching}
					<span class="spinner" role="status" aria-label={m.picker_searching()}></span>
				{/if}
			</div>
			<button
				type="submit"
				class="btn"
				disabled={query.trim().length < MIN_SEARCH_LEN || searching}
			>
				{searching ? m.picker_searching() : m.picker_search_submit()}
			</button>
		</div>
		<fieldset class="scope">
			<legend>{m.picker_scope_legend()}</legend>
			{#each SCOPES as s (s)}
				<label class="scope-option">
					<input type="radio" name="scope" value={s} bind:group={scope} />
					<span>{scopeLabel(s)}</span>
				</label>
			{/each}
		</fieldset>
	</form>

	{#if tooShort}
		<p class="hint" role="status">{m.picker_too_short()}</p>
	{/if}

	{#if errorMessage}
		<p class="inline-error" role="alert">{errorMessage}</p>
	{:else if unavailable}
		<p class="notice" role="alert">{m.picker_unavailable()}</p>
	{:else if searched && !hasResults}
		<p class="empty" role="status">{m.picker_no_matches()}</p>
	{:else if hasResults}
		<div class="groups">
			{#if characters.length > 0}
				<div class="group">
					<h3 class="group-heading">{m.picker_group_characters()}</h3>
					<ul class="results">
						{#each characters as c (c.eve_character_id)}
							{@const key = `char-${c.eve_character_id}`}
							<li>
								<img
									class="portrait"
									src={characterPortrait(c.eve_character_id)}
									alt=""
									width="32"
									height="32"
									loading="lazy"
								/>
								<span class="result-name">{c.name}</span>
								{@render addForm('character', key, c.name, [
									// Always submit the durable EVE id; submit the internal UUID
									// only when a local row already exists. Unknown → backend mints.
									['eve_entity_id', String(c.eve_character_id)],
									...(c.id != null
										? ([['character_id', c.id]] as [string, string][])
										: [])
								])}
							</li>
						{/each}
					</ul>
				</div>
			{/if}

			{#if corporations.length > 0}
				<div class="group">
					<h3 class="group-heading">{m.picker_group_corporations()}</h3>
					<ul class="results">
						{#each corporations as o (o.eve_entity_id)}
							{@const key = `corp-${o.eve_entity_id}`}
							<li>
								<img
									class="portrait"
									src={corporationLogo(o.eve_entity_id)}
									alt=""
									width="32"
									height="32"
									loading="lazy"
								/>
								<span class="result-name">{o.name}</span>
								{@render addForm('corporation', key, o.name, [
									['eve_entity_id', String(o.eve_entity_id)]
								])}
							</li>
						{/each}
					</ul>
				</div>
			{/if}

			{#if alliances.length > 0}
				<div class="group">
					<h3 class="group-heading">{m.picker_group_alliances()}</h3>
					<ul class="results">
						{#each alliances as o (o.eve_entity_id)}
							{@const key = `ally-${o.eve_entity_id}`}
							<li>
								<img
									class="portrait"
									src={allianceLogo(o.eve_entity_id)}
									alt=""
									width="32"
									height="32"
									loading="lazy"
								/>
								<span class="result-name">{o.name}</span>
								{@render addForm('alliance', key, o.name, [
									['eve_entity_id', String(o.eve_entity_id)]
								])}
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		</div>
	{/if}
</div>

{#snippet addForm(
	memberType: MemberType,
	key: string,
	name: string,
	identifiers: [string, string][]
)}
	<form method="POST" action="?/addMember" use:enhance class="add-inline">
		<input type="hidden" name="member_type" value={memberType} />
		<input type="hidden" name="name" value={name} />
		{#each identifiers as [field, value] (field)}
			<input type="hidden" name={field} {value} />
		{/each}
		<select
			name="permission"
			value={permFor(key)}
			onchange={(e) => (perms[key] = e.currentTarget.value as Permission)}
			aria-label={m.acl_add_permission_aria()}
		>
			{#each permissionsFor(memberType) as p (p)}
				<option value={p}>{permLabel(p)}</option>
			{/each}
		</select>
		<button type="submit" class="btn add">{m.acl_add_submit()}</button>
	</form>
{/snippet}

<style>
	.member-picker {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.search-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.search-row {
		display: flex;
		gap: 8px;
	}
	.scope {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 12px;
		margin: 0;
		padding: 4px 8px;
		border: 0;
		border-radius: 4px;
	}
	/* Keyboard focus anywhere in the radio group highlights the whole group, so a
	   human user can see at a glance that the scope selector is active — clearer
	   than a ring on a single small native radio. */
	.scope:focus-within {
		outline: 2px solid var(--sky);
		outline-offset: 1px;
		background: var(--space-900);
	}
	.scope legend {
		float: left;
		margin: 0 4px 0 0;
		padding: 0;
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--slate-500);
	}
	.scope-option {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		font-size: 0.75rem;
		color: var(--slate-300);
		cursor: pointer;
	}
	.scope-option input {
		accent-color: var(--sky);
		cursor: pointer;
	}
	.search-field {
		position: relative;
		flex: 1;
		display: flex;
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
	/* Active-search indicator: the field border lights up and a spinner shows. */
	.search-form.searching input {
		border-color: var(--sky);
	}
	.spinner {
		position: absolute;
		right: 10px;
		top: 50%;
		transform: translateY(-50%);
		width: 14px;
		height: 14px;
		border: 2px solid var(--space-600);
		border-top-color: var(--sky);
		border-radius: 50%;
		animation: spin 0.7s linear infinite;
	}
	@keyframes spin {
		to {
			transform: translateY(-50%) rotate(360deg);
		}
	}

	.groups {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	.group-heading {
		margin: 0 0 4px;
		font-size: 0.625rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--slate-500);
	}

	.results {
		list-style: none;
		margin: 0;
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
		background: var(--space-800);
	}
	.result-name {
		flex: 1;
		font-size: 0.8125rem;
		color: var(--slate-100);
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.add-inline {
		display: flex;
		align-items: center;
		gap: 6px;
		flex: none;
	}
	.add-inline select {
		padding: 6px 8px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.75rem;
	}
	.add-inline select:focus {
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
	.btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.btn.add {
		padding: 6px 12px;
	}

	.hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--slate-500);
	}
	.notice {
		margin: 0;
		padding: 8px 12px;
		background: rgba(245, 158, 11, 0.08);
		border: 1px solid var(--amber);
		border-radius: 4px;
		color: var(--amber);
		font-size: 0.75rem;
	}
	.empty {
		padding: 12px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}
	.inline-error {
		margin: 0;
		color: var(--red);
		font-size: 0.75rem;
	}
</style>
