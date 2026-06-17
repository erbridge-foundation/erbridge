<script lang="ts">
	// Disposable map-canvas sandbox. PURE STATIC: no +page.server.ts, no loader,
	// no auth (the route is in the layout's public list). It owns the fixture's
	// initial graph + local state and feeds the canvas a SCRIPTED list of SSE-style
	// events (replayed one per "receive update") to simulate the live backend; the
	// reusable MapCanvas is the thing under test. When the backend model converges,
	// the real /maps/[slug] mounts MapCanvas with a loader + a real event stream and
	// this route is deleted (see build-map-canvas-prototype "Disposability").
	import MapCanvas from '$lib/components/MapCanvas.svelte';
	import { initialGraph, initialLocalState, updateEvents } from '$lib/fixtures/map-canvas';
	import type { CombinedGraph, LocalState, MapEvent } from '$lib/map/types';

	// Deep-cloned so the canvas can mutate node positions / its graph without
	// scribbling on the shared fixture module (which other tabs/tests import).
	const serverState = $state<CombinedGraph>(structuredClone(initialGraph));
	let localState = $state<LocalState>(structuredClone(initialLocalState));

	// Simulated SSE: hand the canvas the next scripted event each time, returning
	// null once the script is exhausted (the canvas then does nothing).
	let cursor = 0;
	function nextEvent(): MapEvent | null {
		if (cursor >= updateEvents.length) return null;
		return structuredClone(updateEvents[cursor++]);
	}
</script>

<svelte:head>
	<title>Map canvas (prototype)</title>
</svelte:head>

<main class="proto">
	<MapCanvas mapId="_proto" {serverState} bind:localState {nextEvent} />
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
