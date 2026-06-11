<script lang="ts">
	import { m } from '$lib/paraglide/messages';

	let { onclose, isAdmin = false }: { onclose: () => void; isAdmin?: boolean } = $props();
</script>

<div class="user-menu" role="menu" id="user-menu">
	<a class="item" href="/preferences" role="menuitem" onclick={onclose}>{m.user_menu_preferences()}</a>
	<a class="item" href="/account" role="menuitem" onclick={onclose}>{m.user_menu_account()}</a>
	<a class="item" href="/about" role="menuitem" onclick={onclose}>{m.user_menu_about()}</a>
	{#if isAdmin}
		<hr class="divider" />
		<a class="item" href="/admin" role="menuitem" onclick={onclose}>{m.user_menu_admin()}</a>
	{/if}
	<hr class="divider" />
	<form method="POST" action="/auth/logout" class="logout-form">
		<!-- No onclick={onclose} here: closing the menu unmounts this form via
		     {#if open}, which would detach it mid-click and cancel the POST. The
		     submit navigates to / and tears the menu down anyway. -->
		<button type="submit" class="item" role="menuitem">
			{m.user_menu_logout()}
		</button>
	</form>
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

	/* The logout control is a POST form so it cannot be triggered cross-site;
	   reset the button so it matches the link items above it. */
	.logout-form {
		margin: 0;
	}
	button.item {
		width: 100%;
		border: 0;
		background: none;
		font-family: inherit;
		text-align: left;
		cursor: pointer;
	}

	.divider {
		height: 1px;
		margin: 4px 0;
		background: var(--space-700);
		border: 0;
	}
</style>
