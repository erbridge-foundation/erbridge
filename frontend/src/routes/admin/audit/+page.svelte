<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	// "Load older" link: preserve the active filters, append the keyset cursor.
	let loadMoreHref = $derived.by(() => {
		if (!data.page.next_before) return null;
		const params = new URLSearchParams();
		for (const [k, v] of Object.entries(data.filters)) {
			if (v) params.set(k, v);
		}
		params.set('before', data.page.next_before);
		return `/admin/audit?${params.toString()}`;
	});
</script>

<svelte:head>
	<title>{m.admin_audit_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_audit_heading()}</h1>

<form method="GET" class="filters">
	<label class="field">
		<span>{m.admin_audit_filter_target_name()}</span>
		<input type="text" name="target_name" value={data.filters.target_name} />
	</label>
	<label class="field">
		<span>{m.admin_audit_filter_event_type()}</span>
		<input type="text" name="event_type" value={data.filters.event_type} />
	</label>
	<label class="field">
		<span>{m.admin_audit_filter_target_type()}</span>
		<input type="text" name="target_type" value={data.filters.target_type} />
	</label>
	<label class="field">
		<span>{m.admin_audit_filter_target_id()}</span>
		<input type="text" name="target_id" value={data.filters.target_id} />
	</label>
	<label class="field">
		<span>{m.admin_audit_filter_actor()}</span>
		<input type="text" name="actor" value={data.filters.actor} />
	</label>
	<div class="filter-actions">
		<button type="submit" class="btn">{m.admin_audit_filter_apply()}</button>
		<a href="/admin/audit" class="btn ghost">{m.admin_audit_filter_clear()}</a>
	</div>
</form>

<section class="panel">
	{#if data.page.entries.length === 0}
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
				{#each data.page.entries as entry (entry.id)}
					<tr>
						<td class="muted">{new Date(entry.occurred_at).toLocaleString()}</td>
						<td>
							{#if entry.actor_character_name}
								{entry.actor_character_name}
							{:else if entry.actor_account_id}
								<span class="mono">{entry.actor_account_id}</span>
							{:else}
								<span class="muted">{m.admin_audit_actor_system()}</span>
							{/if}
						</td>
						<td><code class="event">{entry.event_type}</code></td>
						<td>
							{#if entry.target_name}
								{entry.target_name}
							{:else if entry.target_id}
								<span class="mono">{entry.target_type ?? ''} {entry.target_id}</span>
							{:else}
								<span class="muted">—</span>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</section>

{#if loadMoreHref}
	<div class="pager">
		<a href={loadMoreHref} class="btn ghost" data-sveltekit-noscroll>{m.admin_audit_load_more()}</a>
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

	.filters {
		display: flex;
		gap: 12px;
		flex-wrap: wrap;
		align-items: flex-end;
		margin-bottom: 16px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.6875rem;
		color: var(--slate-400);
	}
	.field input {
		padding: 6px 10px;
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
	.filter-actions {
		display: flex;
		gap: 8px;
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

	.pager {
		display: flex;
		justify-content: center;
		margin-top: 16px;
	}
</style>
