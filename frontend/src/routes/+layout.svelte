<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import GlobalNav from '$lib/components/GlobalNav.svelte';
	import { preferences } from '$lib/preferences/store.svelte';
	import { coercePreferences } from '$lib/preferences/schema';
	import type { LayoutData } from './$types';

	let { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();

	let isLoginRoute = $derived($page.url.pathname === '/login');

	// Hydrate the preference store from localStorage (the app.html inline script
	// already applied the same values before paint, so this does not re-flash),
	// then reconcile against the authenticated account's server preferences. Runs
	// once per session — not on every client navigation.
	let initialised = false;
	$effect(() => {
		if (initialised) return;
		initialised = true;
		preferences.hydrate();
		const serverPrefs = data.serverPrefs ? coercePreferences(data.serverPrefs) : null;
		void preferences.reconcile(serverPrefs);
	});
</script>

<div class="app" class:login={isLoginRoute}>
	{#if !isLoginRoute}
		<GlobalNav me={data.me} />
		{#if data.meError}
			<div class="layout-error" role="alert">
				Couldn't load your account: {data.meError}
			</div>
		{/if}
	{/if}

	{@render children()}
</div>

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
		overflow: hidden;
		background: var(--space-950);
	}

	.app.login {
		height: 100vh;
		overflow: hidden;
		align-items: center;
		justify-content: center;
	}

	.layout-error {
		flex-shrink: 0;
		padding: 8px 16px;
		background: rgba(239, 68, 68, 0.08);
		border-bottom: 1px solid var(--red);
		color: var(--red);
		font-size: 0.75rem;
	}
</style>
