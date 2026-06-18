<!--
	DialogActions.svelte — the shared footer row for dialogs (the right-aligned button
	cluster). It owns the LAYOUT + the .btn / .btn.ghost / .btn.primary styling that
	Modal consumers (create-map, acls, MapPreferences) were each copy-pasting. It does
	NOT own the buttons or their semantics: the consumer passes them in via `children`,
	so a form dialog can use a `type="submit"` primary while a value dialog uses a
	`type="button"` OK with revert-on-cancel. Place inside a Modal's content.

	Consumers write plain buttons with the shared classes, e.g.:
	    <DialogActions>
	        <button type="button" class="btn ghost" onclick={cancel}>Cancel</button>
	        <button type="submit" class="btn primary">Save</button>
	    </DialogActions>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="dialog-actions">
	{@render children()}
</div>

<style>
	.dialog-actions {
		display: flex;
		justify-content: flex-end;
		gap: 12px;
		margin-top: 4px;
	}
	/* Button styling is :global so the consumer's plain <button class="btn …"> picks it
	   up — the classes are the shared contract. Scoped under .dialog-actions so it can't
	   leak to buttons elsewhere on the page. */
	.dialog-actions :global(.btn) {
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
	.dialog-actions :global(.btn:hover) {
		background: var(--space-700);
	}
	.dialog-actions :global(.btn.ghost) {
		color: var(--slate-400);
	}
	.dialog-actions :global(.btn.primary) {
		background: var(--sky);
		border-color: var(--sky);
		color: var(--space-950);
		font-weight: 600;
	}
	.dialog-actions :global(.btn.primary:hover) {
		background: var(--sky);
		opacity: 0.9;
	}
	.dialog-actions :global(.btn:focus-visible) {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
</style>
