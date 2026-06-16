<script lang="ts">
	// Disposable map-canvas sandbox. PURE STATIC: no +page.server.ts, no loader,
	// no auth (the route is in the layout's public list). It owns the fixture's
	// server-state + local-state and swaps them on "receive update" to simulate
	// SSE; the reusable MapCanvas is the thing under test. When the backend model
	// converges, the real /maps/[slug] mounts MapCanvas with a loader and this
	// route is deleted (see build-map-canvas-prototype "Disposability").
	import MapCanvas from '$lib/components/MapCanvas.svelte';
	import { initialGraph, initialLocalState, updatedGraph } from '$lib/fixtures/map-canvas';
	import type { CombinedGraph, LocalState } from '$lib/map/types';

	// Deep-cloned so the canvas can mutate node positions without scribbling on
	// the shared fixture module (which other tabs/tests import).
	let serverState = $state<CombinedGraph>(structuredClone(initialGraph));
	let localState = $state<LocalState>(structuredClone(initialLocalState));

	// Simulated SSE: swap the server snapshot to the second fixture. The canvas
	// then drops any now-confirmed ghosts and reconciles placement.
	function receiveUpdate() {
		serverState = structuredClone(updatedGraph);
	}
</script>

<svelte:head>
	<title>Map canvas (prototype)</title>
</svelte:head>

<main class="proto">
	<MapCanvas
		mapId="_proto"
		{serverState}
		bind:localState
		onReceiveUpdate={receiveUpdate}
	/>
</main>

<style>
	.proto {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
</style>
