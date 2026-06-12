<script lang="ts">
	import { m } from '$lib/paraglide/messages';

	let { details, open, onClose }: { details: unknown; open: boolean; onClose: () => void } =
		$props();

	let dialog = $state<HTMLDialogElement | null>(null);

	// Drive the native <dialog> open/closed state from the `open` prop. Using the
	// modal API gives us the backdrop, focus trap, and Esc-to-dismiss for free.
	$effect(() => {
		const el = dialog;
		if (!el) return;
		if (open && !el.open) el.showModal();
		else if (!open && el.open) el.close();
	});

	// Flatten the snapshotted `details` payload into key/value rows for a generic
	// render. No id-to-name resolution happens here — the dialog relies on names
	// being snapshotted at write time. Non-object payloads (or `null`) yield no
	// rows, which surfaces the empty state.
	let rows = $derived.by((): { key: string; value: string }[] => {
		if (details === null || typeof details !== 'object') return [];
		return Object.entries(details as Record<string, unknown>).map(([key, value]) => ({
			key,
			value: formatValue(value)
		}));
	});

	function formatValue(value: unknown): string {
		if (value === null || value === undefined) return '—';
		if (typeof value === 'string') return value;
		if (typeof value === 'number' || typeof value === 'boolean') return String(value);
		return JSON.stringify(value);
	}
</script>

<dialog
	bind:this={dialog}
	class="details-dialog"
	aria-label={m.admin_audit_details_title()}
	onclose={onClose}
>
	<div class="dialog-head">
		<h2>{m.admin_audit_details_title()}</h2>
		<button type="button" class="close" aria-label={m.admin_audit_details_close()} onclick={onClose}
			>×</button
		>
	</div>

	{#if rows.length === 0}
		<p class="empty" role="status">{m.admin_audit_details_empty()}</p>
	{:else}
		<dl class="kv">
			{#each rows as row (row.key)}
				<dt>{row.key}</dt>
				<dd>{row.value}</dd>
			{/each}
		</dl>
	{/if}
</dialog>

<style>
	.details-dialog {
		width: min(480px, 92vw);
		padding: 0;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		color: var(--slate-100);
	}
	.details-dialog::backdrop {
		background: rgba(0, 0, 0, 0.6);
	}
	.dialog-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-bottom: 1px solid var(--space-700);
	}
	.dialog-head h2 {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--slate-400);
	}
	.close {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		padding: 0;
		background: transparent;
		border: none;
		border-radius: 4px;
		color: var(--slate-400);
		font-size: 1.125rem;
		line-height: 1;
		cursor: pointer;
	}
	.close:hover {
		background: var(--space-700);
		color: var(--slate-100);
	}
	.kv {
		display: grid;
		grid-template-columns: minmax(0, max-content) 1fr;
		gap: 6px 16px;
		margin: 0;
		padding: 16px;
		font-size: 0.8125rem;
	}
	.kv dt {
		color: var(--slate-500);
		font-family: var(--font-mono, monospace);
		font-size: 0.75rem;
	}
	.kv dd {
		margin: 0;
		color: var(--slate-100);
		word-break: break-word;
	}
	.empty {
		margin: 0;
		padding: 24px 16px;
		text-align: center;
		color: var(--slate-500);
		font-size: 0.75rem;
	}
</style>
