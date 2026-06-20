<script lang="ts">
	// A small right-click context menu for a signature row (Edit / Delete). Follows
	// the pointer rather than anchoring under a chip, so it takes fixed x/y coords.
	// Reuses the UserMenu dropdown pattern: role="menu" with menuitem buttons, closes
	// on an outside click or Escape. PROTOTYPE: sidebar Signatures rows only — canvas
	// right-click is a separate, later concern.
	import { m } from '$lib/paraglide/messages';

	let {
		x,
		y,
		onEdit,
		onDelete,
		onClose
	}: {
		/** Viewport coordinates to anchor the menu at (the click position). */
		x: number;
		y: number;
		onEdit: () => void;
		onDelete: () => void;
		onClose: () => void;
	} = $props();

	let menuEl = $state<HTMLDivElement | null>(null);

	function onDocumentPointerDown(e: PointerEvent) {
		if (menuEl && !menuEl.contains(e.target as Node)) onClose();
	}
	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	$effect(() => {
		// Use pointerdown (capture) so the close fires before a fresh row's
		// contextmenu re-opens it elsewhere.
		document.addEventListener('pointerdown', onDocumentPointerDown, true);
		document.addEventListener('keydown', onKeydown);
		menuEl?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
		return () => {
			document.removeEventListener('pointerdown', onDocumentPointerDown, true);
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<div
	bind:this={menuEl}
	class="sig-menu"
	role="menu"
	style="left: {x}px; top: {y}px;"
>
	<button type="button" class="item" role="menuitem" onclick={onEdit}>
		{m.map_proto_sig_menu_edit()}
	</button>
	<button type="button" class="item danger" role="menuitem" onclick={onDelete}>
		{m.map_proto_sig_menu_delete()}
	</button>
</div>

<style>
	.sig-menu {
		position: fixed;
		min-width: 140px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
		padding: 4px;
		z-index: 1100;
	}
	.item {
		display: block;
		width: 100%;
		padding: 7px 12px;
		font: inherit;
		font-size: 0.75rem;
		text-align: left;
		color: var(--slate-200);
		background: none;
		border: 0;
		border-radius: 4px;
		cursor: pointer;
	}
	.item:hover {
		background: var(--space-700);
	}
	.item.danger {
		color: var(--alert-danger);
	}
	.item:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}
</style>
