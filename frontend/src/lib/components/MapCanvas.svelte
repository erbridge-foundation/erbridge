<script lang="ts">
	// The reusable map canvas. Consumes a POSITION-LESS combined graph + local
	// state and renders it through svelte-flow. Positions come from the layout
	// seed overlaid with saved placement — existence is never derived from
	// placement. This component is the durable artifact; /maps/_proto is the
	// throwaway shell around it (see build-map-canvas-prototype design).
	import { SvelteFlow, Controls, Background, MiniMap, MarkerType } from '@xyflow/svelte';
	import '@xyflow/svelte/dist/style.css';
	import type { Node, Edge, NodeTypes, EdgeTypes } from '@xyflow/svelte';
	import { untrack } from 'svelte';
	import { m } from '$lib/paraglide/messages';

	import SystemNode from '$lib/components/map/SystemNode.svelte';
	import ConnectionEdge from '$lib/components/map/ConnectionEdge.svelte';
	import MapSidebar from '$lib/components/map/MapSidebar.svelte';
	import { layoutSeed, renderableSystems } from '$lib/map/layout';
	import { combine, dropConfirmedGhosts, overlayPositions, reconcilePlacement } from '$lib/map/reconcile';
	import * as placement from '$lib/map/placement';
	import { k162End } from '$lib/map/types';
	import type { CombinedGraph, LayoutDirection, LocalState, Positions, Tab } from '$lib/map/types';

	let {
		mapId,
		serverState,
		localState = $bindable(),
		onReceiveUpdate
	}: {
		mapId: string;
		serverState: CombinedGraph;
		localState: LocalState;
		/** Sandbox SSE simulation: the host swaps server state + reruns reconcile. */
		onReceiveUpdate?: () => void;
	} = $props();

	const nodeTypes: NodeTypes = { system: SystemNode };
	const edgeTypes: EdgeTypes = { connection: ConnectionEdge };

	const tabs = $derived(serverState.tabs);
	// Initial active tab only; tab switching reassigns it. The tab set is stable
	// across the sandbox's server-state swap, so seeding from the initial value
	// is correct here.
	// svelte-ignore state_referenced_locally
	let activeTabId = $state(serverState.tabs[0]?.id ?? '');
	const activeTab = $derived<Tab>(
		tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? { id: '', label: '', roots: [] }
	);

	/** The union graph (server ∪ local). Existence truth for the active tab. */
	const union = $derived(combine(serverState, localState));
	const rootSet = $derived(new Set(activeTab.roots));

	// Active direction is per-render only: applying a layout is a one-shot action
	// (clears saved placement, reseeds) — there is no persistent layout MODE.
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

	// ── Sidebar (holds the intel sections + canvas tweaks; collapses + docks) ────
	let sidebarOpen = $state(true);
	let sidebarSide = $state<'left' | 'right'>('right');
	function flipSidebar(): void {
		sidebarSide = sidebarSide === 'right' ? 'left' : 'right';
	}

	// Saved placement per tab. Seeded once from the store (placement.loadTab); a
	// drag-save or redo-layout reassigns the active tab's entry so the nodes
	// reflow. A version counter lets a redo-layout force a fresh layout pass.
	let savedByTab = $state<Record<string, Positions>>({});

	// Lazy-load the active tab's saved placement from localStorage exactly once,
	// in an effect (NOT in a derived — mutating state mid-derivation is unsafe).
	$effect(() => {
		const id = activeTab.id;
		if (id && !(id in savedByTab)) {
			savedByTab[id] = placement.loadTab(mapId, id);
		}
	});

	const presentIds = $derived(renderableSystems(union, activeTab, localState.ghostSystems));
	const ghostIds = $derived(new Set(localState.ghostSystems.map((s) => s.id)));

	// Positions = saved ?? seed, computed over the union. Reconciled against the
	// current render set so departed nodes' saved positions are dropped. PURE
	// (no state mutation) so it's safe to read while syncing nodes below.
	const positions = $derived.by<Positions>(() => {
		const saved = savedByTab[activeTab.id] ?? {};
		const reconciled = reconcilePlacement(saved, presentIds);
		return overlayPositions(serverState, activeTab, localState, reconciled, 'LR');
	});

	// The desired node/edge sets are PURE deriveds; an effect below syncs them
	// into the bindable $state svelte-flow mutates on drag.
	const desiredNodes = $derived<Node[]>(
		union.systems
			.filter((s) => presentIds.has(s.id))
			.map((s) => ({
				id: s.id,
				type: 'system',
				position: positions[s.id] ?? { x: 0, y: 0 },
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
			// Built-in Svelte Flow arrowhead (MarkerType.ArrowClosed), tangent-
			// accurate and node-hugging. Endpoints are a→b, so arrowTo='b' is the
			// target end (markerEnd) and 'a' is the source end (markerStart). The
			// undetermined case (null) gets no marker — the edge draws a neutral
			// mid-edge diamond instead.
			const colour = c.eol ? 'var(--mass-critical)' : `var(--mass-${c.mass})`;
			const marker = { type: MarkerType.ArrowClosed, color: colour };
			return {
				id: c.id,
				type: 'connection',
				source: c.a.system,
				target: c.b.system,
				markerEnd: arrowTo === 'b' ? marker : undefined,
				markerStart: arrowTo === 'a' ? marker : undefined,
				data: {
					wh_type: namedType,
					mass: c.mass,
					eol: c.eol,
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

	// ── Drag → save (debounced) + one-shot collision repel ──────────────────────
	// Custom collision resolution (NOT svelte-flow proximity-connect — a drag must
	// never assert graph truth). After a drag settles, nudge overlapping nodes
	// apart, then persist the tab's placement.
	const NODE_W = 150;
	const NODE_H = 70;

	function resolveCollisions(): void {
		for (let pass = 0; pass < 4; pass++) {
			let moved = false;
			for (let i = 0; i < nodes.length; i++) {
				for (let j = i + 1; j < nodes.length; j++) {
					const a = nodes[i];
					const b = nodes[j];
					const dx = b.position.x - a.position.x;
					const dy = b.position.y - a.position.y;
					const overlapX = NODE_W - Math.abs(dx);
					const overlapY = NODE_H - Math.abs(dy);
					if (overlapX > 0 && overlapY > 0) {
						// Push apart along the lesser-overlap axis (smallest shove).
						if (overlapX < overlapY) {
							const push = (overlapX / 2) * (dx < 0 ? -1 : 1);
							a.position.x -= push;
							b.position.x += push;
						} else {
							const push = (overlapY / 2) * (dy < 0 ? -1 : 1);
							a.position.y -= push;
							b.position.y += push;
						}
						moved = true;
					}
				}
			}
			if (!moved) break;
		}
	}

	function persistPlacement(): void {
		const next: Positions = {};
		for (const n of nodes) next[n.id] = { x: n.position.x, y: n.position.y };
		savedByTab[activeTab.id] = next;
		placement.save(mapId, activeTab.id, next);
	}

	function handleDragStop(): void {
		resolveCollisions();
		persistPlacement();
	}

	// ── Redo layout (one-shot) ───────────────────────────────────────────────────
	// Clear the saved overlay for the active tab, reseed from the roots, persist.
	// The node-sync effect preserves LIVE positions for kept nodes (so a drag-save
	// doesn't get clobbered), so a reseed must apply the new positions to `nodes`
	// directly — not just to savedByTab.
	function redoLayout(dir: LayoutDirection): void {
		placement.clearTab(mapId, activeTab.id);
		const seed = layoutSeed(union, activeTab, dir, presentIds);
		savedByTab[activeTab.id] = { ...seed };
		placement.save(mapId, activeTab.id, seed, 0);
		nodes = nodes.map((n) => (seed[n.id] ? { ...n, position: { ...seed[n.id] } } : n));
		layoutOpen = false;
	}

	function selectTab(id: string): void {
		activeTabId = id;
	}

	// ── Simulated SSE ────────────────────────────────────────────────────────────
	// The host swaps serverState; we drop any now-confirmed ghosts so the union
	// dedupes (no duplicate / flicker) and reconcile prunes departed placements.
	function receiveUpdate(): void {
		onReceiveUpdate?.();
		localState = dropConfirmedGhosts(serverState, localState);
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
		<div class="flow" data-testid="map-flow">
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
