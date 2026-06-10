<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head>
	<title>{data.map.name}</title>
</svelte:head>

<!-- The canvas fills the whole area. The map name lives in the browser tab
     title (above); map settings are reached from the /maps list's edit control. -->
<main class="canvas">
	<p class="placeholder">{m.maps_canvas_coming_soon()}</p>
</main>

<style>
	.canvas {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		overflow: hidden;
		background:
			radial-gradient(circle at 1px 1px, var(--space-800) 1px, transparent 0) 0 0 / 24px 24px;
	}
	.placeholder {
		color: var(--slate-500);
		font-size: 0.875rem;
		margin: 0;
	}
</style>
