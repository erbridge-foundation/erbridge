<script lang="ts">
	// The reusable map canvas. Consumes a POSITION-LESS combined graph + local
	// state and renders it through svelte-flow, following the Svelte Flow website
	// model: the graph is laid out ONCE on initial load, and thereafter only
	// changes through discrete SSE events (`MapEvent`). An added node is placed
	// incrementally (one flow-step from its anchor, then collisions ripple); there
	// is NO whole-map re-layout, and positions are ephemeral (a refresh re-lays-
	// out — Fork 1 reversed). Existence is never derived from placement. This
	// component is the durable artifact; /maps/_proto is the throwaway shell
	// around it (see build-map-canvas-prototype design).
	import { SvelteFlow, Controls, Background, MiniMap } from '@xyflow/svelte';
	import '@xyflow/svelte/dist/style.css';
	import type { Node, Edge, NodeTypes, EdgeTypes } from '@xyflow/svelte';
	import { untrack } from 'svelte';
	import { m } from '$lib/paraglide/messages';

	import SystemNode from '$lib/components/map/SystemNode.svelte';
	import ConnectionEdge from '$lib/components/map/ConnectionEdge.svelte';
	import MapSidebar from '$lib/components/map/MapSidebar.svelte';
	import { layoutSeed, renderableSystems } from '$lib/map/layout';
	import { combine, dropConfirmedGhosts } from '$lib/map/reconcile';
	import { resolveCollisions } from '$lib/map/resolve-collisions';
	import { placeIncoming } from '$lib/map/place-incoming';
	import { k162End } from '$lib/map/types';
	import type {
		CombinedGraph,
		Connection,
		LayoutDirection,
		LocalState,
		MapEvent,
		Positions,
		System,
		Tab
	} from '$lib/map/types';

	let {
		mapId,
		serverState,
		localState = $bindable(),
		nextEvent
	}: {
		mapId: string;
		serverState: CombinedGraph;
		localState: LocalState;
		/** Sandbox SSE simulation: returns the next scripted event (or null when the
		 *  script is exhausted). The canvas applies it incrementally. */
		nextEvent?: () => MapEvent | null;
	} = $props();

	const nodeTypes: NodeTypes = { system: SystemNode };
	const edgeTypes: EdgeTypes = { connection: ConnectionEdge };

	// EXISTENCE truth, owned by the canvas. Seeded ONCE from the initial server
	// snapshot; thereafter mutated only by applying SSE events (never re-derived
	// from a prop, so a drag/event-driven position is never clobbered by a reactive
	// graph swap). The server is truth; this mirrors it as events arrive.
	// svelte-ignore state_referenced_locally
	let graph = $state<CombinedGraph>(serverState);

	const tabs = $derived(graph.tabs);
	// Initial active tab only; tab switching reassigns it.
	// svelte-ignore state_referenced_locally
	let activeTabId = $state(serverState.tabs[0]?.id ?? '');
	const activeTab = $derived<Tab>(
		tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? { id: '', label: '', roots: [] }
	);

	/** The union graph (server ∪ local). Existence truth for the active tab. */
	const union = $derived(combine(graph, localState));
	const rootSet = $derived(new Set(activeTab.roots));

	// The map's flow direction. Set by the one-shot initial layout and by a
	// "redo layout" action; it tells `placeIncoming` which way a new node steps.
	let layoutDir = $state<LayoutDirection>('LR');
	let layoutOpen = $state(false);

	// ── Display controls (prototype-only, no persistence) ───────────────────────
	// Edge thickness is corp-tunable so people can find a value they like; the
	// label toggles let them see the map with/without the mass + wh-type text.
	// None of this is saved yet — the per-map/account/a11y settings model is a
	// Track-2 decision (see the encoding brainstorm).
	const THICKNESS_MIN = 1;
	const THICKNESS_MAX = 8;
	let edgeThickness = $state(2);
	let showMassLabels = $state(true);
	let showWhTypeLabels = $state(true);
	// "Show direction": a single arrow per connection toward the K162 end (or a
	// neutral marker when the direction is undetermined). On by default.
	let showDirection = $state(true);
	// Colour-blind palette toggle (prototype A/B switch). Swaps ONLY the three mass
	// hues, via a `data-edge-palette` attribute on the canvas wrapper that the
	// app.css token override keys off — see the edge-encoding spec §2.
	let colourblindPalette = $state(false);

	// ── Sidebar (holds the intel sections + canvas tweaks; collapses + docks) ────
	let sidebarOpen = $state(true);
	let sidebarSide = $state<'left' | 'right'>('right');
	function flipSidebar(): void {
		sidebarSide = sidebarSide === 'right' ? 'left' : 'right';
	}

	const presentIds = $derived(renderableSystems(union, activeTab, localState.ghostSystems));
	const ghostIds = $derived(new Set(localState.ghostSystems.map((s) => s.id)));

	// Seed positions per tab. Computed ONCE per tab the first time it is viewed
	// (the one-shot initial layout); a redo-layout reassigns the active tab's
	// entry. An SSE add writes the incoming node's slot here too. Drag positions
	// are NOT mirrored back here — once a node is live, svelte-flow owns its
	// position; `seedPos` only supplies the FIRST position a node ever gets.
	let seedByTab = $state<Record<string, Positions>>({});

	// Lay out the active tab exactly once, in an effect (NOT a derived — mutating
	// state mid-derivation is unsafe). This is the one-shot initial layout.
	$effect(() => {
		const id = activeTab.id;
		if (id && !(id in untrack(() => seedByTab))) {
			seedByTab[id] = layoutSeed(union, activeTab, untrack(() => layoutDir), presentIds);
		}
	});

	const seedPos = $derived<Positions>(seedByTab[activeTab.id] ?? {});

	// The desired node/edge sets are PURE deriveds; an effect below syncs them
	// into the bindable $state svelte-flow mutates on drag. A new node takes its
	// seed slot; kept nodes keep their live (svelte-flow-owned) position.
	const desiredNodes = $derived<Node[]>(
		union.systems
			.filter((s) => presentIds.has(s.id))
			.map((s) => ({
				id: s.id,
				type: 'system',
				position: seedPos[s.id] ?? { x: 0, y: 0 },
				data: { system: s, isRoot: rootSet.has(s.id), isGhost: ghostIds.has(s.id) }
			}))
	);
	const desiredEdges = $derived.by<Edge[]>(() => {
		const visible = union.connections.filter(
			(c) => presentIds.has(c.a.system) && presentIds.has(c.b.system)
		);

		// Parallel-edge detection: two systems can have more than one wormhole
		// between them (dual connections). Group by the UNORDERED pair so we can
		// tell each edge how many siblings it has and which slot it is — the edge
		// component bows parallel siblings apart so they don't stack/overlap.
		const pairKey = (a: string, b: string) => (a < b ? `${a}|${b}` : `${b}|${a}`);
		const groups = new Map<string, string[]>();
		for (const c of visible) {
			const k = pairKey(c.a.system, c.b.system);
			(groups.get(k) ?? groups.set(k, []).get(k)!).push(c.id);
		}

		return visible.map((c) => {
			const siblings = groups.get(pairKey(c.a.system, c.b.system))!;
			// Direction (derived): arrow points toward the K162 end. Edge endpoints
			// are kept as a→b for stable layout; `arrowTo` tells which way to point
			// ('a'|'b'), or null when direction is undetermined.
			const arrowTo = showDirection ? k162End(c) : null;
			// The midpoint label shows the meaningful (named) type, falling back to
			// whatever type is known.
			const namedType =
				(c.a.sig?.type && c.a.sig.type !== 'K162' && c.a.sig.type) ||
				(c.b.sig?.type && c.b.sig.type !== 'K162' && c.b.sig.type) ||
				c.a.sig?.type ||
				c.b.sig?.type ||
				'';
			// No endpoint arrowhead: direction is a → glyph the edge component draws
			// just outside the named end (it derives the named end from `arrowTo`).
			return {
				id: c.id,
				type: 'connection',
				source: c.a.system,
				target: c.b.system,
				data: {
					wh_type: namedType,
					mass: c.mass,
					eol: c.eol,
					ttl_remaining_min: c.ttl_remaining_min,
					sig_a: c.a.sig?.id,
					sig_b: c.b.sig?.id,
					arrowTo,
					showDirection,
					thickness: edgeThickness,
					showMass: showMassLabels,
					showWhType: showWhTypeLabels,
					parallelIndex: siblings.indexOf(c.id),
					parallelCount: siblings.length
				}
			};
		});
	});

	// svelte-flow binds (and mutates positions on drag) into these. The effect
	// reassigns from the pure deriveds; the reassignment is a plain write to
	// $state, not a mutation during another rune's derivation.
	let nodes = $state<Node[]>([]);
	let edges = $state<Edge[]>([]);

	// The system the intel sections describe = the canvas selection. Svelte Flow
	// flips `node.selected` on click (we bind `nodes`), so we read it back here;
	// with nothing selected we fall back to the active tab's first root.
	const selectedId = $derived(nodes.find((n) => n.selected)?.id);
	const selectedSystem = $derived(
		union.systems.find((s) => s.id === selectedId) ??
			union.systems.find((s) => s.id === activeTab.roots[0]) ??
			union.systems[0] ??
			null
	);

	// Reconcile the desired node set INTO the live array rather than replacing it,
	// so Svelte-Flow-owned per-node state (selection, drag) survives a rebuild.
	// A wholesale `nodes = desiredNodes` clobbers `selected` (and would drop drag
	// state) every time placement saves on drag-stop — that's the selection bug.
	// We update data/position on kept nodes, add new ones at their seed, and drop
	// departed ones; existing nodes keep their live position (Svelte Flow owns it).
	$effect(() => {
		const desired = desiredNodes;
		// Read the live array WITHOUT depending on it (untrack) — this effect must
		// react to `desiredNodes` only, not to its own write to `nodes`.
		const live = untrack(() => nodes);
		const byId = new Map(live.map((n) => [n.id, n]));
		nodes = desired.map((dn) => {
			const cur = byId.get(dn.id);
			if (!cur) return dn; // new node → take the seed position + data
			// Kept node: preserve its live position + selection, refresh data.
			return { ...cur, type: dn.type, data: dn.data };
		});
	});
	$effect(() => {
		edges = desiredEdges;
	});

	// ── Collision repel (official @xyflow algorithm) ────────────────────────────
	// Run on drag-stop and after an SSE add. NOT svelte-flow proximity-connect — a
	// drag/add must never assert graph truth, only nudge overlapping nodes apart.
	// It moves whatever it must to clear overlaps, so existing nodes shift to make
	// room ("let it ripple"). margin 15 keeps a small gap between nodes.
	const COLLISION_OPTS = { maxIterations: 1000, overlapThreshold: 0.5, margin: 15 };

	function repel(): void {
		nodes = resolveCollisions(nodes, COLLISION_OPTS);
	}

	function handleDragStop(): void {
		// A drag settles; nudge anything it now overlaps apart. Positions are
		// session-only — svelte-flow owns them and nothing persists them.
		repel();
	}

	// ── Redo layout (one-shot) ───────────────────────────────────────────────────
	// Re-run the one-shot layout for the active tab in a new direction. The
	// node-sync effect preserves LIVE positions for kept nodes (so it can't reflow
	// them on its own), so we apply the fresh seed to `nodes` directly AND update
	// the tab's seed map. `layoutDir` updates so subsequent SSE adds step the new way.
	function redoLayout(dir: LayoutDirection): void {
		layoutDir = dir;
		const seed = layoutSeed(union, activeTab, dir, presentIds);
		seedByTab[activeTab.id] = { ...seed };
		nodes = nodes.map((n) => (seed[n.id] ? { ...n, position: { ...seed[n.id] } } : n));
		layoutOpen = false;
	}

	function selectTab(id: string): void {
		activeTabId = id;
	}

	// ── Simulated SSE ────────────────────────────────────────────────────────────
	// Pull the next scripted event from the host and apply it to the canvas's own
	// graph, placing incrementally — never a whole-map re-layout.
	function applyEvent(ev: MapEvent): void {
		switch (ev.kind) {
			case 'add-system':
				addSystem(ev.system, ev.anchor, ev.connection);
				break;
			case 'add-connection':
				graph = { ...graph, connections: [...graph.connections, ev.connection] };
				break;
			case 'remove-system':
				removeSystem(ev.id);
				break;
			case 'remove-connection':
				graph = { ...graph, connections: graph.connections.filter((c) => c.id !== ev.id) };
				break;
		}
	}

	// Add a system reached through `anchor`: drop it one flow-step out from the
	// anchor's CURRENT (live) position, then resolve collisions over the whole
	// graph so it ripples its neighbours apart. If the system was a local ghost it
	// is dropped from local state (the union then dedupes — no duplicate).
	function addSystem(system: System, anchor: string, connection: Connection): void {
		const anchorPos = nodes.find((n) => n.id === anchor)?.position ?? seedPos[anchor] ?? { x: 0, y: 0 };
		// Seed the incoming node BEFORE it enters the render set, so the node-sync
		// effect places it there rather than at the origin.
		seedByTab[activeTab.id] = { ...seedPos, [system.id]: placeIncoming(anchorPos, layoutDir) };
		// Mutate existence truth; the union + deriveds pick the new node/edge up.
		const exists = graph.systems.some((s) => s.id === system.id);
		graph = {
			...graph,
			systems: exists ? graph.systems : [...graph.systems, system],
			connections: graph.connections.some((c) => c.id === connection.id)
				? graph.connections
				: [...graph.connections, connection]
		};
		localState = dropConfirmedGhosts(graph, localState);
		// The node-sync effect flushes the new node into `nodes` reactively; ripple
		// once that has happened (next microtask), so collisions see the real node.
		queueMicrotask(repel);
	}

	function removeSystem(id: string): void {
		graph = {
			...graph,
			systems: graph.systems.filter((s) => s.id !== id),
			connections: graph.connections.filter((c) => c.a.system !== id && c.b.system !== id)
		};
		// Forget its seed slot so a future re-add re-places it.
		if (id in seedPos) {
			const next = { ...seedPos };
			delete next[id];
			seedByTab[activeTab.id] = next;
		}
	}

	function receiveUpdate(): void {
		const ev = nextEvent?.();
		if (ev) applyEvent(ev);
	}
</script>

<div class="map-canvas">
	<!-- Tabs: local UI state, multi-root + wildcard are just tabs with roots/flags. -->
	<nav class="tabs" aria-label={m.map_proto_tabs_label()}>
		{#each tabs as tab (tab.id)}
			<button
				type="button"
				class="tab"
				class:active={tab.id === activeTabId}
				aria-pressed={tab.id === activeTabId}
				onclick={() => selectTab(tab.id)}
			>
				{tab.label}
			</button>
		{/each}
	</nav>

	<!-- Canvas + a docked, collapsible sidebar (System Intel / Signatures / Pilots
	     / Structures + Map Canvas Tweaks). `data-side` flips which edge it docks to;
	     the sidebar-outer animates WIDTH on collapse (wireframe slide), so the
	     content stays mounted and the canvas reflows smoothly. -->
	<div class="stage" data-side={sidebarSide}>
		<div
			class="flow"
			data-testid="map-flow"
			data-edge-palette={colourblindPalette ? 'colourblind' : 'standard'}
		>
			<SvelteFlow
				bind:nodes
				bind:edges
				{nodeTypes}
				{edgeTypes}
				fitView
				nodesConnectable={false}
				onnodedragstop={handleDragStop}
				proOptions={{ hideAttribution: true }}
				aria-label={m.map_proto_canvas_label()}
			>
				<Background />
				<Controls />
				<!-- Dark-themed minimap: our node colour, dark mask + surface (the default
				     light theme washes out against the dark canvas). -->
				<MiniMap
					bgColor="var(--space-900)"
					maskColor="rgba(5, 8, 15, 0.7)"
					nodeColor="var(--space-600)"
					nodeStrokeColor="var(--sky)"
				/>
			</SvelteFlow>
		</div>

		<div class="sidebar-outer" class:collapsed={!sidebarOpen} data-side={sidebarSide}>
			<!-- Collapse/expand toggle, overflowing the sidebar's inner edge. -->
			<button
				type="button"
				class="sidebar-toggle"
				aria-expanded={sidebarOpen}
				aria-label={sidebarOpen ? m.map_proto_sidebar_close() : m.map_proto_sidebar_open()}
				title={sidebarOpen ? m.map_proto_sidebar_close() : m.map_proto_sidebar_open()}
				onclick={() => (sidebarOpen = !sidebarOpen)}
			>
				<span class="sidebar-toggle-icon" aria-hidden="true">
					{sidebarSide === 'right' ? '›' : '‹'}
				</span>
			</button>

			<aside class="sidebar" aria-label={m.map_proto_sidebar_label()}>
				<header class="sidebar-head">
					<button
						type="button"
						class="icon-btn"
						aria-label={m.map_proto_sidebar_flip()}
						title={m.map_proto_sidebar_flip()}
						onclick={flipSidebar}
					>
						⇄
					</button>
				</header>

				<MapSidebar
					selected={selectedSystem}
					bind:thickness={edgeThickness}
					thicknessMin={THICKNESS_MIN}
					thicknessMax={THICKNESS_MAX}
					bind:showMass={showMassLabels}
					bind:showWhType={showWhTypeLabels}
					bind:showDirection
					bind:colourblind={colourblindPalette}
					bind:layoutOpen
					onRedoLayout={redoLayout}
					onReceiveUpdate={receiveUpdate}
				/>
			</aside>
		</div>
	</div>
</div>

<style>
	.map-canvas {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
	.tabs {
		display: flex;
		gap: 0.25rem;
		padding: 0.4rem 0.6rem;
		background: var(--space-900);
		border-bottom: 1px solid var(--space-700);
	}
	.tab {
		padding: 0.25rem 0.7rem;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-300);
		font-family: var(--font-ui);
		font-size: 0.75rem;
		cursor: pointer;
	}
	.tab.active {
		background: var(--space-700);
		color: var(--slate-100);
		border-color: var(--sky);
	}
	.tab:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	/* Stage: canvas + docked sidebar. `data-side` flips the flex order so the
	   sidebar sits on the chosen edge. */
	.stage {
		flex: 1;
		display: flex;
		min-height: 0;
	}
	.stage[data-side='left'] {
		flex-direction: row-reverse;
	}

	.flow {
		flex: 1;
		min-height: 0;
		position: relative;
	}

	/* Sidebar outer wrapper: animates WIDTH on collapse (wireframe slide). The
	   content stays mounted; collapsing shrinks it to a thin rail. Positioned so
	   the toggle can overflow the inner edge without being clipped. */
	.sidebar-outer {
		flex: none;
		position: relative;
		display: flex;
		width: 288px;
		transition: width 0.2s ease;
	}
	.sidebar-outer.collapsed {
		width: 14px;
	}
	@media (prefers-reduced-motion: reduce) {
		.sidebar-outer {
			transition: none;
		}
	}

	.sidebar {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		background: var(--space-900);
		overflow-y: auto;
		overflow-x: hidden;
	}
	/* Collapsed: hide the panel content (the rail is just the toggle). */
	.sidebar-outer.collapsed .sidebar {
		visibility: hidden;
	}
	/* Borders on the edge that meets the canvas, per dock side. */
	.sidebar-outer[data-side='right'] .sidebar {
		border-left: 1px solid var(--space-700);
	}
	.sidebar-outer[data-side='left'] .sidebar {
		border-right: 1px solid var(--space-700);
	}

	/* Collapse/expand toggle, a round button overflowing the inner edge. */
	.sidebar-toggle {
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		z-index: 20;
		width: 24px;
		height: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--space-800);
		border: 1px solid var(--space-600);
		border-radius: 50%;
		color: var(--slate-400);
		cursor: pointer;
		transition:
			color 0.15s,
			background 0.15s;
	}
	.sidebar-outer[data-side='right'] .sidebar-toggle {
		left: -12px;
	}
	.sidebar-outer[data-side='left'] .sidebar-toggle {
		right: -12px;
	}
	.sidebar-toggle:hover {
		color: var(--slate-200);
		background: var(--space-700);
	}
	.sidebar-toggle:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.sidebar-toggle-icon {
		font-size: 0.9rem;
		line-height: 1;
		transition: transform 0.2s ease;
	}
	.sidebar-outer.collapsed .sidebar-toggle-icon {
		transform: rotate(180deg);
	}
	@media (prefers-reduced-motion: reduce) {
		.sidebar-toggle-icon {
			transition: none;
		}
	}

	.sidebar-head {
		display: flex;
		justify-content: flex-end;
		padding: 6px 8px;
		border-bottom: 1px solid var(--space-700);
	}
	.icon-btn {
		width: 1.6rem;
		height: 1.6rem;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-200);
		font-size: 0.85rem;
		cursor: pointer;
	}
	.icon-btn:hover {
		background: var(--space-700);
	}
	.icon-btn:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	/* The svelte-flow default theme is light; pin its surfaces to our dark tokens. */
	.flow :global(.svelte-flow) {
		background: var(--space-950);
	}
	.flow :global(.svelte-flow__minimap) {
		background: var(--space-900);
	}
	/* Zoom/fit controls: the default white buttons glare against the dark canvas. */
	.flow :global(.svelte-flow__controls) {
		box-shadow: none;
	}
	.flow :global(.svelte-flow__controls-button) {
		background: var(--space-800);
		border-bottom: 1px solid var(--space-700);
		color: var(--slate-100);
		fill: var(--slate-100);
	}
	.flow :global(.svelte-flow__controls-button:hover) {
		background: var(--space-700);
	}
	.flow :global(.svelte-flow__controls-button svg) {
		fill: currentColor;
	}
</style>
