<script lang="ts">
	import { page } from '$app/stores';
	import { m } from '$lib/paraglide/messages';
	import UserChip from './UserChip.svelte';
	import StatusIcon from './StatusIcon.svelte';
	import type { MeResponse } from '$lib/api';

	let { me }: { me: MeResponse | null } = $props();

	let main = $derived(me?.characters.find((c) => c.is_main) ?? null);
	let connected = $derived(me !== null);
	let isAdmin = $derived(me?.account.is_server_admin ?? false);
</script>

<header class="global-nav">
	<a href="/" class="brand">
		<svg
			width="18"
			height="18"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.5"
			aria-hidden="true"
		>
			<circle cx="12" cy="12" r="3"></circle>
			<path d="M12 2v4M12 18v4M2 12h4M18 12h4"></path>
			<path d="M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M18.4 5.6l-2.8 2.8M8.4 15.6l-2.8 2.8"></path>
		</svg>
		<span class="brand-name">E-R BRIDGE</span>
	</a>

	<nav class="nav-links">
		<a href="/maps" class:active={$page.url.pathname.startsWith('/maps')}>{m.nav_maps()}</a>
		<a href="/acls" class:active={$page.url.pathname.startsWith('/acls')}>{m.nav_acls()}</a>
		<a href="/characters" class:active={$page.url.pathname === '/characters'}>{m.nav_characters()}</a>
	</nav>

	<div class="nav-spacer"></div>

	<div class="status" aria-live="polite">
		<StatusIcon level={connected ? 'ok' : 'error'} />
		<span>{connected ? m.nav_connected() : m.nav_disconnected()}</span>
	</div>

	{#if main}
		<UserChip portraitUrl={main.portrait_url} name={main.name} {isAdmin} />
	{:else}
		<div class="user-chip-placeholder" aria-label={m.nav_not_signed_in()}></div>
	{/if}
</header>

<style>
	.global-nav {
		height: 48px;
		display: flex;
		align-items: center;
		padding: 0 16px;
		background: var(--space-900);
		border-bottom: 1px solid var(--space-700);
		flex-shrink: 0;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		padding-right: 16px;
		border-right: 1px solid var(--space-700);
		text-decoration: none;
		color: var(--slate-100);
	}
	.brand :global(svg) {
		color: var(--sky);
	}
	.brand-name {
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
	}

	.nav-links {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-left: 16px;
	}
	.nav-links a {
		display: inline-flex;
		align-items: center;
		padding: 6px 10px;
		border-radius: 4px;
		font-size: 0.75rem;
		color: var(--slate-400);
		text-decoration: none;
	}
	.nav-links a:hover {
		color: var(--slate-200);
	}
	.nav-links a.active {
		color: var(--sky);
		background: var(--space-700);
	}

	.nav-spacer {
		flex: 1;
	}

	.status {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		margin-right: 16px;
		font-size: 0.75rem;
		color: var(--slate-400);
	}

	.user-chip-placeholder {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--space-700);
	}
</style>
