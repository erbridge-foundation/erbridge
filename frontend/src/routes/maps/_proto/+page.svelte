<script lang="ts">
	// Disposable map-canvas sandbox. PURE STATIC: no +page.server.ts, no loader,
	// no auth (the route is in the layout's public list). It owns the fixture's
	// initial graph + local state and feeds the canvas a SCRIPTED list of SSE-style
	// events (replayed one per "receive update") to simulate the live backend; the
	// reusable MapCanvas is the thing under test. When the backend model converges,
	// the real /maps/[slug] mounts MapCanvas with a loader + a real event stream and
	// this route is deleted (see build-map-canvas-prototype "Disposability").
	import MapCanvas from '$lib/components/MapCanvas.svelte';
	import NodeLab from '$lib/components/map/_lab/NodeLab.svelte';
	import { m } from '$lib/paraglide/messages';
	import { initialGraph, initialLocalState, updateEvents } from '$lib/fixtures/map-canvas';
	import type { CombinedGraph, LocalState, MapEvent } from '$lib/map/types';

	// Top-level view switch: the real canvas vs the disposable Node-lab wireframe gallery.
	// Kept at the PAGE level (not a MapCanvas tab) because the lab is a different surface —
	// a static comparison grid, not a graph view. Removed with the lab.
	let view = $state<'canvas' | 'lab'>('canvas');

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
	<!-- View switch (disposable, alongside the Node lab). A segmented toggle so the
	     wireframe gallery is one click from the canvas without route plumbing. -->
	<div class="view-switch" role="group" aria-label={m.map_proto_view_label()}>
		<button
			type="button"
			class:active={view === 'canvas'}
			aria-pressed={view === 'canvas'}
			onclick={() => (view = 'canvas')}
		>
			{m.map_proto_view_canvas()}
		</button>
		<button
			type="button"
			class:active={view === 'lab'}
			aria-pressed={view === 'lab'}
			onclick={() => (view = 'lab')}
		>
			{m.map_proto_view_lab()}
		</button>
	</div>

	{#if view === 'canvas'}
		<MapCanvas mapId="_proto" {serverState} bind:localState {nextEvent} />
	{:else}
		<NodeLab />
	{/if}
</main>

<style>
	.proto {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
	.view-switch {
		display: flex;
		gap: 2px;
		padding: 6px 8px;
		background: var(--space-900);
		border-bottom: 1px solid var(--space-700);
	}
	.view-switch button {
		padding: 4px 12px;
		font-size: 0.78rem;
		font-weight: 600;
		color: var(--slate-400);
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 5px;
		cursor: pointer;
	}
	.view-switch button:hover {
		color: var(--slate-200);
	}
	.view-switch button.active {
		color: var(--space-950, #000);
		background: var(--sky);
		border-color: var(--sky);
	}
	.view-switch button:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
</style>
