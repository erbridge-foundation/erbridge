<script lang="ts">
	import { m } from '$lib/paraglide/messages';
</script>

<svelte:head>
	<title>{m.maps_title()}</title>
</svelte:head>

<main class="body">
	<p class="placeholder">{m.maps_coming_soon()}</p>
</main>

<style>
	.body {
		flex: 1;
		overflow: auto;
		display: flex;
		justify-content: center;
		align-items: center;
		padding: 24px;
	}

	.placeholder {
		color: var(--slate-500);
		font-size: 0.875rem;
		margin: 0;
	}
</style>
