<script lang="ts">
	import { page } from '$app/stores';
	import { m } from '$lib/paraglide/messages';

	let { children }: { children: import('svelte').Snippet } = $props();

	let path = $derived($page.url.pathname);
</script>

<main class="admin-body">
	<div class="admin-content">
		<nav class="admin-tabs" aria-label="Admin sections">
			<a href="/admin" class:active={path === '/admin'}>{m.admin_nav_overview()}</a>
			<a href="/admin/admins" class:active={path === '/admin/admins'}>{m.admin_nav_admins()}</a>
			<a href="/admin/blocks" class:active={path === '/admin/blocks'}>{m.admin_nav_blocks()}</a>
			<a href="/admin/audit" class:active={path === '/admin/audit'}>{m.admin_nav_audit()}</a>
		</nav>

		{@render children()}
	</div>
</main>

<style>
	.admin-body {
		flex: 1;
		overflow: auto;
		display: flex;
		justify-content: center;
		padding: 32px 24px 48px;
	}
	.admin-content {
		width: 100%;
		max-width: 960px;
	}

	.admin-tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 24px;
		border-bottom: 1px solid var(--space-700);
	}
	.admin-tabs a {
		padding: 8px 12px;
		font-size: 0.75rem;
		color: var(--slate-400);
		text-decoration: none;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
	}
	.admin-tabs a:hover {
		color: var(--slate-200);
	}
	.admin-tabs a.active {
		color: var(--sky);
		border-bottom-color: var(--sky);
	}
</style>
