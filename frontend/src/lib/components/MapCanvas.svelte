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
	import MapPreferences from '$lib/components/map/MapPreferences.svelte';
	import LayoutMenu from '$lib/components/map/LayoutMenu.svelte';
	import MapLegend from '$lib/components/map/MapLegend.svelte';
	import { layoutSeed, renderableSystems } from '$lib/map/layout';
	import { combine, dropConfirmedGhosts } from '$lib/map/reconcile';
	import { danglingStubs, isDanglingId } from '$lib/map/dangling';
	import { resolveCollisions } from '$lib/map/resolve-collisions';
	import { placeIncoming } from '$lib/map/place-incoming';
	import { k162End } from '$lib/map/types';
	import type {
		CombinedGraph,
		Connection,
		LayoutAlgorithm,
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
		tabs.find((t) => t.id === activeTabId) ?? tabs[0] ?? { id: '', label: '', root: '' }
	);

	/** The union graph (server ∪ local), with dangling stubs folded in. Existence
	 *  truth for the active tab. A scanned-but-unjumped wormhole sig (a wormhole scan
	 *  no connection references) gets a faint stub node on the far end (see
	 *  dangling.ts); deriving it here lets the stubs flow through reachability +
	 *  layout + edge rendering exactly like real neighbours, with no special-casing. */
	const baseUnion = $derived(combine(graph, localState));
	const stubs = $derived(danglingStubs(baseUnion));
	const union = $derived<CombinedGraph>(
		stubs.systems.length === 0
			? baseUnion
			: {
					systems: [...baseUnion.systems, ...stubs.systems],
					connections: [...baseUnion.connections, ...stubs.connections],
					tabs: baseUnion.tabs
				}
	);

	// The map's flow direction. Set by the one-shot initial layout and by a
	// "redo layout" action; it tells `placeIncoming` which way a new node steps.
	// The selected layout style (persists; the segmented control sets it). Feeds the
	// initial seed, the manual "Redo layout" button, incremental new-node placement
	// (`placeIncoming` steps in this direction), and the auto-layout reflow.
	// Default TB (top-to-bottom): the tidy-tree layout reads best vertically for the
	// deep, wide chains these maps grow into (the root sits top-centre, branches fan
	// down symmetrically). The user can switch via the tab-bar layout menu.
	let layoutDir = $state<LayoutDirection>('TB');
	// Which layout ENGINE seeds positions (a user preference, set in the prefs dialog).
	// Default 'dagre' (the layered look); 'tidy-tree' is the corp's Wanderer look. Both
	// honour the same contract; changing it reflows the active tab (the effect below).
	let layoutAlgo = $state<LayoutAlgorithm>('dagre');
	// Auto-layout: when ON, every map change re-runs the whole layout in the selected
	// style (drags discarded — layout is machine-owned). When OFF, a map change keeps
	// existing positions and only the new node is placed incrementally (today's
	// behaviour). Session-only, like the other prototype prefs.
	let autoLayout = $state(false);

	// ── Display controls (prototype-only, no persistence) ───────────────────────
	// Edge thickness is corp-tunable so people can find a value they like; the
	// label toggles let them see the map with/without the mass + wh-type text.
	// None of this is saved yet — the per-map/account/a11y settings model is a
	// Track-2 decision (see the encoding brainstorm).
	const THICKNESS_MIN = 1;
	const THICKNESS_MAX = 8;
	let edgeThickness = $state(2);
	// Node spacing: a cross-axis layout multiplier (percent) the user can tune so a
	// busy chain — e.g. a system fanning out several dangling stubs — spreads apart
	// instead of overlapping. Applied at (re)layout time; 100% = the compact dagre base
	// spacing, which is the default — dagre already lays the chain out tightly and
	// crossing-free, so we start compact and let the user spread up to 250% on demand.
	const SPACING_MIN = 100;
	const SPACING_MAX = 250;
	let nodeSpacing = $state(100);
	// Mass + wh-type label text default OFF — the canvas reads less cluttered out of
	// the box (the mass colour/thickness + the legend already carry the encoding), and
	// people who want the text turn it on in the prefs dialog.
	let showMassLabels = $state(false);
	let showWhTypeLabels = $state(false);
	// "Show signatures": the per-end sig-id pills on each connection (which signature
	// in each system leads to the hole). On by default.
	let showSignatures = $state(true);
	// "Show direction": a single arrow per connection toward the K162 end (or a
	// neutral marker when the direction is undetermined). On by default.
	let showDirection = $state(true);
	// "Animate direction": when on, the direction arrow drifts along the line toward
	// its destination. A TASTE preference (default OFF — constant page motion annoys),
	// SEPARATE from prefers-reduced-motion, which is an additional backstop that always
	// freezes it. So it animates only when BOTH this is on AND the OS allows motion.
	let animateDirection = $state(false);
	// Colour-blind palette toggle (prototype A/B switch). Swaps ONLY the three mass
	// hues, via a `data-edge-palette` attribute on the STAGE wrapper (covers both the
	// canvas edges and the legend swatches) that the app.css token override keys off
	// — see the edge-encoding spec §2.
	let colourblindPalette = $state(false);
	// Map Preferences dialog (the cog on the tab bar). Holds the display PREFERENCES
	// (thickness, label toggles, layout style + auto); session-only for now.
	let prefsOpen = $state(false);

	// ── Sidebar (holds the intel sections + canvas tweaks; collapses + docks) ────
	let sidebarOpen = $state(true);
	let sidebarSide = $state<'left' | 'right'>('right');
	// Legend: a show/hide key pinned to the sidebar bottom (starts collapsed).
	let legendOpen = $state(false);
	function flipSidebar(): void {
		if (locked) return;
		sidebarSide = sidebarSide === 'right' ? 'left' : 'right';
	}

	// Bulk collapse/expand of the sidebar sections, driven from the header. The
	// per-section open state lives in MapSidebar, so we nudge an incrementing signal
	// it watches rather than reaching into its state. Collapse-all ALSO collapses
	// the legend (the user asked for it to honour collapse-all only — expand-all
	// leaves the legend as the user set it).
	let collapseAllSignal = $state(0);
	let expandAllSignal = $state(0);
	function collapseAll(): void {
		collapseAllSignal++;
		legendOpen = false;
	}
	function expandAll(): void {
		expandAllSignal++;
	}

	// Lock the whole arrangement: freezes the layout gestures (flip, resize) and the
	// section toggles, so a tuned panel can't be disturbed by a stray click. Purely
	// a UI guard (session-only, prototype).
	let locked = $state(false);

	// ── Resizable sidebar ────────────────────────────────────────────────────────
	// User-draggable width (session-only, like the other prototype prefs). A gripper
	// on the inner edge (the one meeting the canvas) drives this; clamped so it can't
	// swallow the canvas or collapse to nothing. The drag direction depends on the
	// dock side: pulling toward the canvas widens it either way.
	const SIDEBAR_MIN = 220;
	// Comfortable default ceiling. Power users can pull WIDER — up to half the
	// viewport — so the real max is dynamic (see SIDEBAR_MAX). 560 is just the floor
	// for that max, so on a narrow window it never drops below the old comfortable
	// size and stays > SIDEBAR_MIN.
	const SIDEBAR_MAX_FLOOR = 560;
	// Track the viewport so "up to half the screen" stays correct across resizes.
	let innerWidth = $state(typeof window === 'undefined' ? 1280 : window.innerWidth);
	const SIDEBAR_MAX = $derived(Math.max(SIDEBAR_MAX_FLOOR, Math.round(innerWidth / 2)));
	let sidebarWidth = $state(288);
	let resizing = $state(false);

	const clampWidth = (w: number) => Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w));

	// If the viewport shrinks below twice the current width, pull the sidebar back in
	// so it never exceeds half the (now smaller) screen.
	$effect(() => {
		if (sidebarWidth > SIDEBAR_MAX) sidebarWidth = SIDEBAR_MAX;
	});

	function startResize(ev: PointerEvent): void {
		if (locked) return;
		ev.preventDefault();
		resizing = true;
		const startX = ev.clientX;
		const startW = sidebarWidth;
		// Right-docked: dragging LEFT (negative dx) widens. Left-docked: mirror.
		const dir = sidebarSide === 'right' ? -1 : 1;
		const onMove = (e: PointerEvent) => {
			sidebarWidth = clampWidth(startW + dir * (e.clientX - startX));
		};
		const onUp = () => {
			resizing = false;
			window.removeEventListener('pointermove', onMove);
			window.removeEventListener('pointerup', onUp);
		};
		window.addEventListener('pointermove', onMove);
		window.addEventListener('pointerup', onUp);
	}

	// Keyboard resize on the separator: ←/→ nudge, Home/End jump to the bounds. The
	// dock side decides which arrow widens (toward the canvas).
	function resizeKey(ev: KeyboardEvent): void {
		if (locked) return;
		const step = ev.shiftKey ? 32 : 8;
		const widen = sidebarSide === 'right' ? 'ArrowLeft' : 'ArrowRight';
		const narrow = sidebarSide === 'right' ? 'ArrowRight' : 'ArrowLeft';
		if (ev.key === widen) sidebarWidth = clampWidth(sidebarWidth + step);
		else if (ev.key === narrow) sidebarWidth = clampWidth(sidebarWidth - step);
		else if (ev.key === 'Home') sidebarWidth = SIDEBAR_MIN;
		else if (ev.key === 'End') sidebarWidth = SIDEBAR_MAX;
		else return;
		ev.preventDefault();
	}

	const presentIds = $derived(renderableSystems(union, activeTab, localState.ghostSystems));
	const ghostIds = $derived(new Set(localState.ghostSystems.map((s) => s.id)));

	// Seed positions per tab. Computed ONCE per tab the first time it is viewed
	// (the one-shot initial layout); a redo-layout reassigns the active tab's
	// entry. An SSE add writes the incoming node's slot here too. Drag positions
	// are NOT mirrored back here — once a node is live, svelte-flow owns its
	// position; `seedPos` only supplies the FIRST position a node ever gets.
	let seedByTab = $state<Record<string, Positions>>({});

	// Each tab is its own placement snowflake: the live positions (seed + drags +
	// ripples) a node has WHILE a tab is active are remembered against that tab, so
	// leaving and returning restores that tab's arrangement instead of dragging the
	// previous tab's layout along. A system shared by two tabs can therefore sit in
	// a different spot in each. Session-only (like seeds) — a reload re-lays-out.
	let posByTab = $state<Record<string, Positions>>({});

	// Lay out the active tab exactly once, in an effect (NOT a derived — mutating
	// state mid-derivation is unsafe). This is the one-shot initial layout.
	$effect(() => {
		const id = activeTab.id;
		if (id && !(id in untrack(() => seedByTab))) {
			seedByTab[id] = layoutSeed(
				union,
				activeTab,
				untrack(() => layoutDir),
				presentIds,
				untrack(() => nodeSpacing) / 100,
				untrack(() => layoutAlgo)
			);
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
				data: {
					system: s,
					isRoot: s.id === activeTab.root,
					isGhost: ghostIds.has(s.id),
					isDangling: isDanglingId(s.id),
					danglingDest: stubs.dest.get(s.id) ?? null
				}
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
			// No endpoint arrowhead: direction is a → arrow the edge component draws at
			// the midpoint, rotated along the line (it derives the named end from `arrowTo`).
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
					// Sig pills are gated by the toggle: when off, simply withhold the ids
					// and the edge renders no endpoint pills (no new edge prop needed).
					sig_a: showSignatures ? c.a.sig?.id : undefined,
					sig_b: showSignatures ? c.b.sig?.id : undefined,
					arrowTo,
					showDirection,
					animateDirection,
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
	// with nothing selected we fall back to the active tab's root.
	const selectedId = $derived(nodes.find((n) => n.selected)?.id);
	const selectedSystem = $derived(
		union.systems.find((s) => s.id === selectedId) ??
			union.systems.find((s) => s.id === activeTab.root) ??
			union.systems[0] ??
			null
	);

	// Which tab the live `nodes` array currently reflects. A change means the user
	// switched tabs, so the node-sync effect restores the new tab's snowflake
	// instead of carrying the old tab's live positions over.
	// svelte-ignore state_referenced_locally
	let renderedTabId = $state(activeTabId);

	// Reconcile the desired node set INTO the live array rather than replacing it,
	// so Svelte-Flow-owned per-node state (selection, drag) survives a rebuild.
	// A wholesale `nodes = desiredNodes` clobbers `selected` (and would drop drag
	// state) every time placement saves on drag-stop — that's the selection bug.
	// We update data/position on kept nodes, add new ones at their seed, and drop
	// departed ones; existing nodes keep their live position (Svelte Flow owns it).
	//
	// On a TAB SWITCH the carried-over live array belongs to the OLD tab, so we
	// first snapshot it against that tab (`posByTab`) and then place each node from
	// the NEW tab's remembered positions (or its seed) — making each tab its own
	// placement snowflake rather than letting one tab's drags follow into another.
	$effect(() => {
		const desired = desiredNodes;
		const id = activeTab.id;
		const switched = id !== untrack(() => renderedTabId);

		// Read the live array WITHOUT depending on it (untrack) — this effect must
		// react to `desiredNodes` (and the tab id) only, not to its own write.
		const live = untrack(() => nodes);

		if (switched) {
			// Remember the outgoing tab's live arrangement before leaving it.
			const prev = untrack(() => renderedTabId);
			if (prev) {
				const snapshot: Positions = {};
				for (const n of live) snapshot[n.id] = { ...n.position };
				posByTab[prev] = snapshot;
			}
			// Rebuild from the incoming tab's remembered positions, falling back to
			// its seed. Selection is tab-local too, so it does not carry over.
			const remembered = untrack(() => posByTab)[id] ?? {};
			renderedTabId = id;
			nodes = desired.map((dn) => ({
				...dn,
				position: remembered[dn.id] ?? dn.position
			}));
			return;
		}

		// Same tab: preserve each kept node's live position + selection, refresh data.
		const byId = new Map(live.map((n) => [n.id, n]));
		nodes = desired.map((dn) => {
			const cur = byId.get(dn.id);
			if (!cur) return dn; // new node → take the seed position + data
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

	// ── Layout ───────────────────────────────────────────────────────────────────
	// Re-run the whole layout for the active tab in the selected style. The node-sync
	// effect preserves LIVE positions for kept nodes (so it can't reflow them on its
	// own), so we apply the fresh seed to `nodes` directly AND update the tab's seed
	// map. Any manual drags for this tab are discarded — a reflow is machine-owned.
	function reflow(): void {
		const seed = layoutSeed(union, activeTab, layoutDir, presentIds, nodeSpacing / 100, layoutAlgo);
		seedByTab[activeTab.id] = { ...seed };
		nodes = nodes.map((n) => (seed[n.id] ? { ...n, position: { ...seed[n.id] } } : n));
		// Drop the remembered arrangement for this tab so leaving and returning shows
		// the reflowed layout, not the pre-reflow one. The next switch away re-snapshots.
		delete posByTab[activeTab.id];
	}

	// Node-spacing slider: a spacing change only matters once it is APPLIED to the
	// layout, so reflow the active tab when the value changes (mirrors the layout-
	// style picker, which also reflows on change). Skip the initial run so the first
	// view still uses its one-shot seed. A reflow is machine-owned, so it discards any
	// manual drags on the active tab — the same trade-off the style picker makes.
	// svelte-ignore state_referenced_locally
	let lastSpacing = nodeSpacing;
	$effect(() => {
		const s = nodeSpacing;
		if (s !== untrack(() => lastSpacing)) {
			lastSpacing = s;
			reflow();
		}
	});

	// Layout-algorithm picker (prefs dialog): switching engines is a whole-map re-seed,
	// so reflow the active tab when it changes (same machine-owned reflow as the spacing
	// slider — discards drags). Skip the initial run; the first view uses its one-shot
	// seed (already computed with the default algorithm).
	// svelte-ignore state_referenced_locally
	let lastAlgo = layoutAlgo;
	$effect(() => {
		const a = layoutAlgo;
		if (a !== untrack(() => lastAlgo)) {
			lastAlgo = a;
			reflow();
		}
	});

	// The segmented control sets the selected style. With auto-layout ON, changing the
	// style immediately reflows; OFF, it just records the choice (the manual Redo
	// button or the next incremental placement will use it).
	function selectLayout(dir: LayoutDirection): void {
		layoutDir = dir;
		if (autoLayout) reflow();
	}

	function selectTab(id: string): void {
		activeTabId = id;
	}

	// ── Simulated SSE ────────────────────────────────────────────────────────────
	// Pull the next scripted event from the host and apply it to the canvas's own
	// graph. With auto-layout OFF this places incrementally (a new node steps out
	// from its anchor; existing drags are kept). With auto-layout ON the whole map
	// is reflowed in the selected style after the change lands.
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
		// Auto-layout: reflow the whole map once the graph mutation has flushed into
		// the derived union/presentIds (next microtask), so layout sees the new set.
		if (autoLayout) queueMicrotask(reflow);
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

<svelte:window bind:innerWidth />

<div class="map-canvas">
	<!-- Tab bar: the root tabs on the left, the map-preferences cog on the right. -->
	<div class="tab-bar">
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

		<!-- Layout split-button: apply the current style now (icon = current style) +
		     a caret dropdown to pick LR/RL/TB/BT. Sits left of the preferences cog. -->
		<LayoutMenu
			{layoutDir}
			onSelect={selectLayout}
			onApply={reflow}
			disabled={locked}
		/>

		<!-- Map preferences: a cog opening the per-user display-prefs dialog. -->
		<button
			type="button"
			class="prefs-cog"
			aria-label={m.map_proto_prefs_open()}
			title={m.map_proto_prefs_open()}
			onclick={() => (prefsOpen = true)}
			disabled={locked}
		>
			<svg
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="1.8"
				stroke-linecap="round"
				stroke-linejoin="round"
				aria-hidden="true"
			>
				<circle cx="12" cy="12" r="3" />
				<path
					d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
				/>
			</svg>
		</button>
	</div>

	<!-- Canvas + a docked, collapsible sidebar (System Intel / Signatures / Pilots
	     / Structures + Map Canvas Tweaks). `data-side` flips which edge it docks to;
	     the sidebar-outer animates WIDTH on collapse (wireframe slide), so the
	     content stays mounted and the canvas reflows smoothly. -->
	<!-- `data-edge-palette` sits on the STAGE (not just the flow) so the colour-blind
	     mass-hue swap in app.css cascades to BOTH the canvas edges AND the legend
	     swatches in the sidebar — the legend reads the same --mass-* tokens, so it
	     must live under the same palette scope to recolour in lock-step. -->
	<div
		class="stage"
		data-side={sidebarSide}
		data-testid="map-stage"
		data-edge-palette={colourblindPalette ? 'colourblind' : 'standard'}
	>
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

		<div
			class="sidebar-outer"
			class:collapsed={!sidebarOpen}
			class:resizing
			data-side={sidebarSide}
			style={sidebarOpen ? `width: ${sidebarWidth}px;` : ''}
		>
			<!-- Resize gripper on the inner edge (meets the canvas). Drag to widen/
			     narrow; arrow keys nudge for keyboard users. Hidden when collapsed. -->
			{#if sidebarOpen}
				<!-- A focusable, operable window splitter is a legitimate `separator`
				     widget (drag + arrow-key resize); Svelte's a11y lint flags any
				     `separator` as non-interactive, so silence it for this element. -->
				<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
				<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
				<div
					class="sidebar-resizer"
					role="separator"
					aria-orientation="vertical"
					aria-label={m.map_proto_sidebar_resize()}
					aria-valuenow={sidebarWidth}
					aria-valuemin={SIDEBAR_MIN}
					aria-valuemax={SIDEBAR_MAX}
					tabindex="0"
					onpointerdown={startResize}
					onkeydown={resizeKey}
				></div>
			{/if}
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
				<!-- Scrolling region: the top-down intel sections. It is the flex
				     child that yields, so the pinned legend below expands upward. -->
				<div class="sidebar-scroll">
				<header class="sidebar-head">
					<!-- Collapse all: a chevron pointing UP to a bar below it (sections fold
					     up to a line). The bar is an unmistakable horizontal element, so it
					     never reads as an "X" the way two bare chevrons can. -->
					<button
						type="button"
						class="icon-btn"
						aria-label={m.map_proto_sidebar_collapse_all()}
						title={m.map_proto_sidebar_collapse_all()}
						onclick={collapseAll}
						disabled={locked}
					>
						<svg
							class="head-icon"
							viewBox="0 0 16 16"
							fill="none"
							stroke="currentColor"
							stroke-width="1.8"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path d="M4 7l4-4 4 4" />
							<path d="M4 12h8" />
						</svg>
					</button>
					<!-- Expand all: a chevron pointing DOWN from a bar above it (sections
					     unfold down from a line). -->
					<button
						type="button"
						class="icon-btn"
						aria-label={m.map_proto_sidebar_expand_all()}
						title={m.map_proto_sidebar_expand_all()}
						onclick={expandAll}
						disabled={locked}
					>
						<svg
							class="head-icon"
							viewBox="0 0 16 16"
							fill="none"
							stroke="currentColor"
							stroke-width="1.8"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path d="M4 4h8" />
							<path d="M4 9l4 4 4-4" />
						</svg>
					</button>
					<button
						type="button"
						class="icon-btn"
						class:active={locked}
						aria-label={locked
							? m.map_proto_sidebar_unlock()
							: m.map_proto_sidebar_lock()}
						title={locked ? m.map_proto_sidebar_unlock() : m.map_proto_sidebar_lock()}
						aria-pressed={locked}
						onclick={() => (locked = !locked)}
					>
						{locked ? '🔒' : '🔓'}
					</button>
					<button
						type="button"
						class="icon-btn"
						aria-label={m.map_proto_sidebar_flip()}
						title={m.map_proto_sidebar_flip()}
						onclick={flipSidebar}
						disabled={locked}
					>
						⇄
					</button>
				</header>

				<MapSidebar
					selected={selectedSystem}
					bind:colourblind={colourblindPalette}
					{collapseAllSignal}
					{expandAllSignal}
					{locked}
					onRedoLayout={reflow}
					onReceiveUpdate={receiveUpdate}
				/>
				</div>

				<!-- Legend: pinned footer, expands upward (see MapLegend). Honours
				     collapse-all (collapseAll sets legendOpen=false); frozen when locked. -->
				<MapLegend bind:open={legendOpen} {locked} />
			</aside>
		</div>
	</div>
</div>

<!-- Map Preferences dialog (opened by the tab-bar cog). Its edits bind back to the
     canvas state and apply live; the blurred backdrop keeps the canvas visible behind
     so the changes preview as you make them. Session-only for now. -->
<MapPreferences
	bind:open={prefsOpen}
	bind:thickness={edgeThickness}
	thicknessMin={THICKNESS_MIN}
	thicknessMax={THICKNESS_MAX}
	bind:nodeSpacing
	spacingMin={SPACING_MIN}
	spacingMax={SPACING_MAX}
	bind:layoutAlgo
	bind:showMass={showMassLabels}
	bind:showWhType={showWhTypeLabels}
	bind:showSignatures
	bind:showDirection
	bind:animateDirection
	bind:autoLayout
/>

<style>
	.map-canvas {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
	/* Tab bar: tabs on the left, the preferences cog pinned to the right. */
	.tab-bar {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.4rem 0.6rem;
		background: var(--space-900);
		border-bottom: 1px solid var(--space-700);
	}
	.tabs {
		display: flex;
		flex: 1;
		gap: 0.25rem;
		min-width: 0;
		overflow-x: auto;
	}
	/* Preferences cog: a quiet icon button at the right end of the tab bar. */
	.prefs-cog {
		flex: none;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-400);
		cursor: pointer;
		transition:
			color 0.15s,
			background 0.15s;
	}
	.prefs-cog svg {
		width: 16px;
		height: 16px;
	}
	.prefs-cog:hover:not(:disabled) {
		color: var(--slate-100);
		background: var(--space-800);
	}
	.prefs-cog:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.prefs-cog:disabled {
		opacity: 0.4;
		cursor: not-allowed;
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
		/* Width is set inline from `sidebarWidth` when open; the collapse animation
		   below overrides it. Defaults here for any no-inline-style fallback. */
		width: 288px;
		transition: width 0.2s ease;
	}
	.sidebar-outer.collapsed {
		width: 14px !important;
	}
	/* During an active drag, kill the width transition so it tracks the pointer. */
	.sidebar-outer.resizing {
		transition: none;
		user-select: none;
	}
	@media (prefers-reduced-motion: reduce) {
		.sidebar-outer {
			transition: none;
		}
	}

	/* Resize gripper: a thin hit-strip on the inner edge that meets the canvas. The
	   docked side decides which edge; widened hit area, slim visible line. */
	.sidebar-resizer {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 8px;
		z-index: 25;
		cursor: ew-resize;
		touch-action: none;
	}
	.sidebar-outer[data-side='right'] .sidebar-resizer {
		left: -4px;
	}
	.sidebar-outer[data-side='left'] .sidebar-resizer {
		right: -4px;
	}
	.sidebar-resizer::after {
		content: '';
		position: absolute;
		top: 0;
		bottom: 0;
		left: 50%;
		width: 1px;
		background: var(--space-700);
		transform: translateX(-50%);
		transition: background 0.15s;
	}
	.sidebar-resizer:hover::after,
	.sidebar-outer.resizing .sidebar-resizer::after {
		background: var(--sky);
		width: 2px;
	}
	.sidebar-resizer:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}

	.sidebar {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		background: var(--space-900);
		overflow: hidden;
	}
	/* The intel sections scroll; the legend is pinned below them (it lives outside
	   this region), so a tall legend never pushes the sections offscreen. */
	.sidebar-scroll {
		flex: 1;
		min-height: 0;
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

	/* Collapse/expand toggle, a round button overflowing the inner edge. It sits ABOVE
	   the resize gripper (z-index 25) so its full circle is clickable — otherwise the
	   1px grab line ran down the middle of the button and split its hit area, forcing a
	   click to either side. The resizer is still grabbable along the rest of the height
	   (the button only occupies a 24px band at mid-height). */
	.sidebar-toggle {
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		z-index: 30;
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
		gap: 4px;
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
	.icon-btn:hover:not(:disabled) {
		background: var(--space-700);
	}
	.head-icon {
		width: 1.05rem;
		height: 1.05rem;
	}
	/* The lock toggle when engaged: accent border so the locked state is visible. */
	.icon-btn.active {
		border-color: var(--sky);
		color: var(--slate-100);
	}
	.icon-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
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
