<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { goto } from '$app/navigation';
	import type { AuditLogEntryDto, AuditLogPageDto } from '$lib/api';
	import {
		EVENT_TYPES,
		TARGET_TYPES,
		WINDOW_TIERS,
		groupByDay,
		isSecurityEvent,
		nextWiderWindow
	} from '$lib/audit';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// Window-tier label lookup (the only window labels surfaced are the four
	// relative tiers; year buckets, if ever set via URL, fall back to the raw
	// value).
	const windowLabels: Record<string, string> = {
		'7d': m.admin_audit_window_7d(),
		'30d': m.admin_audit_window_30d(),
		'90d': m.admin_audit_window_90d(),
		'365d': m.admin_audit_window_365d()
	};
	const targetTypeLabels: Record<string, string> = {
		account: m.admin_audit_target_type_account(),
		character: m.admin_audit_target_type_character(),
		map: m.admin_audit_target_type_map(),
		acl: m.admin_audit_target_type_acl()
	};

	// Accumulated entries across infinite-scroll pages. Seeded from the first
	// page and reset whenever the load `data` changes (new filter navigation).
	let entries = $state<AuditLogEntryDto[]>([]);
	let nextBefore = $state<string | null>(null);
	let loadingMore = $state(false);
	// `data.page` identity changes on each navigation; (re-)seed from it.
	$effect(() => {
		entries = data.page.entries;
		nextBefore = data.page.next_before;
	});

	let groups = $derived(groupByDay(entries));
	const widerWindow = $derived(nextWiderWindow(data.filters.window));

	function dayHeader(key: string): string {
		if (key === 'today') return m.admin_audit_day_today();
		if (key === 'yesterday') return m.admin_audit_day_yesterday();
		return key;
	}

	/** Builds the active-filter query params (everything except the keyset
	 * cursor), starting from the current filter state with `overrides` applied.
	 * A key set to `null` is removed. */
	function buildParams(overrides: Record<string, string | null> = {}): URLSearchParams {
		const base: Record<string, string> = {
			window: data.filters.window,
			event_type: data.filters.event_type,
			actor: data.filters.actor,
			target_type: data.filters.target_type,
			target_id: data.filters.target_id,
			q: data.filters.q
		};
		const merged = { ...base, ...overrides };
		const params = new URLSearchParams();
		for (const [k, v] of Object.entries(merged)) {
			if (v) params.set(k, v);
		}
		return params;
	}

	function applyFilters(overrides: Record<string, string | null>) {
		const params = buildParams(overrides);
		goto(`/admin/audit?${params.toString()}`, { keepFocus: true, noScroll: true });
	}

	// Click-to-refine: replace within a column.
	function refineActor(entry: AuditLogEntryDto) {
		if (!entry.actor_account_id) return;
		applyFilters({ actor: entry.actor_account_id });
	}
	function refineEvent(entry: AuditLogEntryDto) {
		applyFilters({ event_type: entry.event_type });
	}
	function refineTarget(entry: AuditLogEntryDto) {
		if (!entry.target_type || !entry.target_id) return;
		applyFilters({ target_type: entry.target_type, target_id: entry.target_id });
	}

	// Search box (Enter-to-search → q).
	let searchTerm = $state('');
	$effect(() => {
		searchTerm = data.filters.q;
	});
	function submitSearch(event: SubmitEvent) {
		event.preventDefault();
		applyFilters({ q: searchTerm.trim() || null });
	}

	// Select changes navigate immediately.
	function onWindowChange(event: Event) {
		applyFilters({ window: (event.currentTarget as HTMLSelectElement).value });
	}
	function onEventTypeChange(event: Event) {
		applyFilters({ event_type: (event.currentTarget as HTMLSelectElement).value || null });
	}
	function onTargetTypeChange(event: Event) {
		applyFilters({ target_type: (event.currentTarget as HTMLSelectElement).value || null });
	}
	let targetIdInput = $state('');
	$effect(() => {
		targetIdInput = data.filters.target_id;
	});
	function submitTargetId(event: SubmitEvent) {
		event.preventDefault();
		applyFilters({ target_id: targetIdInput.trim() || null });
	}

	// Active chips (label + the override that removes them).
	let chips = $derived.by(() => {
		const out: { key: string; label: string; remove: Record<string, string | null> }[] = [];
		if (data.filters.q) {
			out.push({
				key: 'q',
				label: m.admin_audit_chip_search({ value: data.filters.q }),
				remove: { q: null }
			});
		}
		if (data.filters.event_type) {
			out.push({
				key: 'event_type',
				label: m.admin_audit_chip_event({ value: data.filters.event_type }),
				remove: { event_type: null }
			});
		}
		if (data.filters.actor) {
			out.push({
				key: 'actor',
				label: m.admin_audit_chip_actor({ value: data.filters.actor }),
				remove: { actor: null }
			});
		}
		if (data.filters.target_type || data.filters.target_id) {
			const value = `${data.filters.target_type} ${data.filters.target_id}`.trim();
			out.push({
				key: 'target',
				label: m.admin_audit_chip_target({ value }),
				remove: { target_type: null, target_id: null }
			});
		}
		return out;
	});
	let hasFilters = $derived(chips.length > 0);

	// Infinite scroll: fetch the next older page within the window via the
	// keyset cursor, append, and stop at the window edge.
	async function loadMore() {
		if (loadingMore || !nextBefore) return;
		loadingMore = true;
		try {
			const params = buildParams({});
			params.set('before', nextBefore);
			const res = await fetch(`/admin/audit/more?${params.toString()}`);
			if (!res.ok) return;
			const body = (await res.json()) as { data: AuditLogPageDto };
			entries = [...entries, ...body.data.entries];
			nextBefore = body.data.next_before;
		} finally {
			loadingMore = false;
		}
	}

	// Sentinel observer drives loadMore when it scrolls into view.
	function sentinel(node: HTMLElement) {
		const observer = new IntersectionObserver((obsEntries) => {
			if (obsEntries.some((e) => e.isIntersecting)) loadMore();
		});
		observer.observe(node);
		return { destroy: () => observer.disconnect() };
	}
</script>

<svelte:head>
	<title>{m.admin_audit_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_audit_heading()}</h1>

<div class="controls">
	<label class="field">
		<span>{m.admin_audit_window_label()}</span>
		<select value={data.filters.window} onchange={onWindowChange}>
			{#each WINDOW_TIERS as tier (tier)}
				<option value={tier}>{windowLabels[tier] ?? tier}</option>
			{/each}
		</select>
	</label>

	<form class="field search" onsubmit={submitSearch}>
		<span>{m.admin_audit_search_label()}</span>
		<input
			type="search"
			bind:value={searchTerm}
			placeholder={m.admin_audit_search_placeholder()}
			aria-label={m.admin_audit_search_label()}
		/>
	</form>

	<label class="field">
		<span>{m.admin_audit_filter_event_type()}</span>
		<select value={data.filters.event_type} onchange={onEventTypeChange}>
			<option value="">{m.admin_audit_event_type_any()}</option>
			{#each EVENT_TYPES as et (et)}
				<option value={et}>{et}</option>
			{/each}
		</select>
	</label>

	<label class="field">
		<span>{m.admin_audit_filter_target_type()}</span>
		<select value={data.filters.target_type} onchange={onTargetTypeChange}>
			<option value="">{m.admin_audit_target_type_any()}</option>
			{#each TARGET_TYPES as tt (tt)}
				<option value={tt}>{targetTypeLabels[tt] ?? tt}</option>
			{/each}
		</select>
	</label>

	<form class="field" onsubmit={submitTargetId}>
		<span>{m.admin_audit_filter_target_id()}</span>
		<input type="text" bind:value={targetIdInput} aria-label={m.admin_audit_filter_target_id()} />
	</form>
</div>

{#if hasFilters}
	<div class="chips" role="list">
		{#each chips as chip (chip.key)}
			<span class="chip" role="listitem">
				{chip.label}
				<button
					type="button"
					class="chip-remove"
					aria-label={m.admin_audit_chip_remove()}
					onclick={() => applyFilters(chip.remove)}>×</button
				>
			</span>
		{/each}
		<a href="/admin/audit" class="clear-all">{m.admin_audit_clear_all()}</a>
	</div>
{/if}

<section class="panel">
	{#if entries.length === 0}
		<p class="empty" role="status">{m.admin_audit_empty()}</p>
	{:else}
		<table class="admin-table">
			<thead>
				<tr>
					<th>{m.admin_audit_col_when()}</th>
					<th>{m.admin_audit_col_actor()}</th>
					<th>{m.admin_audit_col_event()}</th>
					<th>{m.admin_audit_col_target()}</th>
				</tr>
			</thead>
			<tbody>
				{#each groups as group (group.key)}
					<tr class="day-header">
						<th colspan="4" scope="colgroup">{dayHeader(group.key)}</th>
					</tr>
					{#each group.entries as entry (entry.id)}
						<tr class:security={isSecurityEvent(entry.event_type)}>
							<td class="muted">{new Date(entry.occurred_at).toLocaleString()}</td>
							<td>
								{#if entry.actor_character_name || entry.actor_account_id}
									<button type="button" class="cell-btn" onclick={() => refineActor(entry)}>
										{#if entry.actor_character_name}
											{entry.actor_character_name}
										{:else}
											<span class="mono">{entry.actor_account_id}</span>
										{/if}
									</button>
								{:else}
									<span class="muted">{m.admin_audit_actor_system()}</span>
								{/if}
							</td>
							<td>
								<button type="button" class="cell-btn" onclick={() => refineEvent(entry)}>
									<code class="event">{entry.event_type}</code>
								</button>
							</td>
							<td>
								{#if entry.target_id}
									<button type="button" class="cell-btn" onclick={() => refineTarget(entry)}>
										{#if entry.target_name}
											{entry.target_name}
										{:else}
											<span class="mono">{entry.target_type ?? ''} {entry.target_id}</span>
										{/if}
									</button>
								{:else}
									<span class="muted">—</span>
								{/if}
							</td>
						</tr>
					{/each}
				{/each}
			</tbody>
		</table>
	{/if}
</section>

{#if nextBefore}
	<div class="sentinel" use:sentinel></div>
{:else if entries.length > 0}
	<div class="window-edge">
		<p class="muted">{m.admin_audit_window_edge()}</p>
		{#if widerWindow}
			<button type="button" class="btn ghost" onclick={() => applyFilters({ window: widerWindow })}>
				{m.admin_audit_widen({ window: windowLabels[widerWindow] ?? widerWindow })}
			</button>
		{/if}
	</div>
{/if}

<style>
	.page-heading {
		margin: 0 0 16px;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-500);
	}

	.controls {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
		align-items: flex-end;
		margin-bottom: 12px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.6875rem;
		color: var(--slate-400);
		margin: 0;
	}
	.field.search {
		flex: 1 1 240px;
	}
	.field input,
	.field select {
		padding: 6px 10px;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8125rem;
	}
	.field input:focus,
	.field select:focus {
		outline: none;
		border-color: var(--sky);
	}

	.chips {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;
		align-items: center;
		margin-bottom: 16px;
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 4px 6px 4px 10px;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 999px;
		font-size: 0.75rem;
		color: var(--slate-200);
	}
	.chip-remove {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: 50%;
		color: var(--slate-400);
		font-size: 0.875rem;
		line-height: 1;
		cursor: pointer;
	}
	.chip-remove:hover {
		background: var(--space-700);
		color: var(--slate-100);
	}
	.clear-all {
		font-size: 0.75rem;
		color: var(--slate-400);
		text-decoration: none;
	}
	.clear-all:hover {
		color: var(--sky);
	}

	.panel {
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		padding: 8px;
		overflow-x: auto;
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
		vertical-align: top;
	}
	.day-header th {
		padding-top: 16px;
		color: var(--slate-300);
		font-size: 0.75rem;
		letter-spacing: 0.08em;
		border-bottom: 1px solid var(--space-700);
	}
	tr.security td .event {
		color: var(--amber);
	}
	tr.security td:first-child {
		border-left: 2px solid var(--amber);
	}
	.muted {
		color: var(--slate-500);
	}
	.mono {
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		color: var(--slate-400);
	}
	.event {
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
		color: var(--sky);
	}
	.cell-btn {
		display: inline-flex;
		padding: 0;
		background: transparent;
		border: none;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.cell-btn:hover {
		text-decoration: underline;
	}

	.btn {
		display: inline-flex;
		align-items: center;
		padding: 7px 14px;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
		text-decoration: none;
		white-space: nowrap;
	}
	.btn:hover {
		background: var(--space-700);
	}
	.btn.ghost {
		color: var(--slate-400);
	}

	.empty {
		padding: 24px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
		margin: 0;
	}

	.sentinel {
		height: 1px;
	}
	.window-edge {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
		margin-top: 16px;
		font-size: 0.75rem;
	}
</style>
