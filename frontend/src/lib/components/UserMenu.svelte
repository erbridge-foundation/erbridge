<script lang="ts">
	import { m } from '$lib/paraglide/messages';

	let { onclose }: { onclose: () => void } = $props();
</script>

<div class="user-menu" role="menu" id="user-menu">
	<a class="item" href="/preferences" role="menuitem" onclick={onclose}>{m.user_menu_preferences()}</a>
	<span class="item disabled" role="menuitem" aria-disabled="true" tabindex="-1"
		>{m.user_menu_settings()}</span
	>
	<a class="item" href="/about" role="menuitem" onclick={onclose}>{m.user_menu_about()}</a>
	<hr class="divider" />
	<a class="item" href="/auth/logout" role="menuitem" onclick={onclose}>{m.user_menu_logout()}</a>
</div>

<style>
	.user-menu {
		position: absolute;
		top: calc(100% + 6px);
		right: 0;
		min-width: 200px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
		padding: 4px;
		z-index: 50;
	}

	.item {
		display: block;
		padding: 8px 12px;
		font-size: 0.75rem;
		color: var(--slate-200);
		text-decoration: none;
		border-radius: 4px;
	}
	.item:hover {
		background: var(--space-700);
	}
	.item.disabled {
		color: var(--slate-600);
		cursor: not-allowed;
	}
	.item.disabled:hover {
		background: transparent;
	}

	.divider {
		height: 1px;
		margin: 4px 0;
		background: var(--space-700);
		border: 0;
	}
</style>
