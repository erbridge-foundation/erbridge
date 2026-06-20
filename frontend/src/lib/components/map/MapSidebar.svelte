<script lang="ts">
	// The map sidebar: collapsible intel sections (System Intel / Signatures /
	// Pilots / Structures) modelled on the wireframe, plus a "Map Canvas Tweaks"
	// section holding the prototype display controls. The intel sections render
	// SAMPLE data for now — they're wired to the chain-map model on the backend
	// track. Lives under components/map/ as part of the canvas theme seam.
	import { m } from '$lib/paraglide/messages';
	import type { ScanResult, Structure, System } from '$lib/map/types';
	import { relativeTime, localAndEveTime } from '$lib/map/relative-time';
	import SigRowMenu from './SigRowMenu.svelte';
	import SigEditDialog from './SigEditDialog.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import DialogActions from '$lib/components/DialogActions.svelte';

	let {
		selected,
		colourblind = $bindable(),
		collapseAllSignal = 0,
		expandAllSignal = 0,
		locked = false,
		onRedoLayout,
		onReceiveUpdate
	}: {
		/** The system the intel sections describe (a stand-in for canvas selection). */
		selected: System | null;
		/** Throwaway colour-blind A/B switch (removed at promotion); kept in Tweaks. */
		colourblind: boolean;
		/** Incrementing signals from the header's collapse-all / expand-all buttons —
		 *  the parent owns the action, this component owns the per-section state. */
		collapseAllSignal?: number;
		expandAllSignal?: number;
		/** When the arrangement is locked, section headers don't toggle. */
		locked?: boolean;
		/** Manually reflow the whole map now, in the selected style ("Apply layout"). */
		onRedoLayout: () => void;
		onReceiveUpdate: () => void;
	} = $props();

	// Per-section open state. Sections start COLLAPSED so the open sidebar presents a
	// tidy list of headers; the user expands the ones they want. (Persisting which
	// sections the user leaves open is a real-route concern — it belongs with the
	// unified per-user prefs storage decision deferred to Track 2, not a localStorage
	// island in the proto.)
	let open = $state({
		intel: false,
		signatures: false,
		pilots: false,
		structures: false,
		tweaks: false
	});
	type SectionKey = keyof typeof open;
	function toggle(k: SectionKey) {
		if (locked) return;
		open[k] = !open[k];
	}
	function setAll(value: boolean) {
		for (const k of Object.keys(open) as SectionKey[]) open[k] = value;
	}

	// React to the header's bulk signals. The signals are counters so a repeat of
	// the same action still fires; the very first values (0) seed without acting.
	// svelte-ignore state_referenced_locally
	let lastCollapse = collapseAllSignal;
	// svelte-ignore state_referenced_locally
	let lastExpand = expandAllSignal;
	$effect(() => {
		if (collapseAllSignal !== lastCollapse) {
			lastCollapse = collapseAllSignal;
			setAll(false);
		}
	});
	$effect(() => {
		if (expandAllSignal !== lastExpand) {
			lastExpand = expandAllSignal;
			setAll(true);
		}
	});

	// Signatures + Structures bind to the SELECTED system. Signatures are now
	// editable (add / rename / delete); Structures stay read-only this phase.
	const scans = $derived<ScanResult[]>(selected?.scans ?? []);
	const structures = $derived<Structure[]>(selected?.structures ?? []);

	// --- Signature add / edit / delete (prototype, session-only) ---------------
	// The proto has no auth/character context, so authored records are stamped with a
	// placeholder character id. This is the seam the real "who edited this" plumbing
	// replaces at promotion.
	const PROTO_CHAR_ID = 0;

	// Right-click context menu state (anchored at the click position).
	let sigMenu = $state<{ scan: ScanResult; x: number; y: number } | null>(null);
	// The add/edit dialog. `dialogScan` is the row being edited (null in add mode).
	let dialogOpen = $state(false);
	let dialogMode = $state<'add' | 'edit'>('add');
	let dialogScan = $state<ScanResult | null>(null);
	// Delete isn't wired to real removal yet (it ties into the event/history model,
	// built later) — for now it surfaces a "not implemented" notice.
	let deleteNoticeOpen = $state(false);

	/** Sig ids already in the selected system — the uniqueness set passed to the
	 *  dialog. In edit mode the edited row's own id is excluded so a no-op rename
	 *  (and changing only the name/type) passes. */
	const existingSigIds = $derived(
		scans
			.filter((s) => s.sig_id !== dialogScan?.sig_id)
			.map((s) => s.sig_id)
	);

	function openAddSig() {
		if (locked) return;
		dialogMode = 'add';
		dialogScan = null;
		dialogOpen = true;
	}
	function openEditSig(scan: ScanResult) {
		dialogMode = 'edit';
		dialogScan = scan;
		dialogOpen = true;
	}
	function openSigMenu(scan: ScanResult, e: MouseEvent) {
		e.preventDefault();
		sigMenu = { scan, x: e.clientX, y: e.clientY };
	}

	function saveSig(fields: { sig_id: string; site_type: string | null; name: string | null }) {
		if (!selected) return;
		const now = new Date().toISOString();
		if (dialogMode === 'edit' && dialogScan) {
			// Mutate the existing record in place (session-only proto state), then
			// reassign the array so the table re-renders.
			dialogScan.sig_id = fields.sig_id;
			dialogScan.site_type = fields.site_type;
			dialogScan.name = fields.name;
			dialogScan.updated_at = now;
			dialogScan.updated_by = PROTO_CHAR_ID;
		} else {
			selected.scans.push({
				sig_id: fields.sig_id,
				group: 'Cosmic Signature',
				site_type: fields.site_type,
				name: fields.name,
				wh_type: null,
				created_at: now,
				created_by: PROTO_CHAR_ID,
				updated_at: now,
				updated_by: PROTO_CHAR_ID
			});
		}
		selected.scans = [...selected.scans];
	}

	function deleteSig() {
		sigMenu = null;
		// STUB: no real removal yet — delete ties into the event/history model later.
		// Surface a "not implemented" notice so the action gives visible feedback.
		deleteNoticeOpen = true;
	}

	/** The Type cell: the site classification when known, else the scanner category
	 *  ("Cosmic Signature" etc.). */
	function scanType(s: ScanResult): string {
		return s.site_type ?? s.group;
	}
	/** This is a wormholers' tool — wormhole sigs read differently from cosmic
	 *  sites, so they get their own colour in the table. */
	function isWormhole(s: ScanResult): boolean {
		return s.site_type === 'Wormhole';
	}
	/** The Info cell: resolved name, the wh-type code for typed wormholes, or a
	 *  generic "Unknown" while the scan is still unidentified. */
	function scanInfo(s: ScanResult): string {
		if (s.name) return s.name;
		if (s.wh_type) return s.wh_type;
		return m.map_proto_sig_unknown();
	}
	/** A full provenance line for the native row tooltip (the custom tooltip
	 *  component is deferred to the paste/CRUD phase). Timestamps are shown in both
	 *  the user's local time AND EVE time (UTC) — see localAndEveTime. */
	function provenance(meta: { created_at: string; created_by: number; updated_at: string; updated_by: number }): string {
		return (
			`Created ${localAndEveTime(meta.created_at)} by ${meta.created_by}\n` +
			`Updated ${localAndEveTime(meta.updated_at)} by ${meta.updated_by}`
		);
	}
	function sourceLabel(src: Structure['source']): string {
		return src === 'scanner'
			? m.map_proto_struct_source_scanner()
			: src === 'dscan'
				? m.map_proto_struct_source_dscan()
				: m.map_proto_struct_source_overview();
	}
	function timerLabel(state: NonNullable<Structure['timer']>['state']): string {
		return state === 'reinforced'
			? m.map_proto_struct_timer_reinforced()
			: state === 'anchoring'
				? m.map_proto_struct_timer_anchoring()
				: m.map_proto_struct_timer_unanchoring();
	}

	// Pilots stay SAMPLE — pilots aren't modelled until the pilot-search work.
	const pilots = [
		{ name: 'Alara Voss', ship: 'Loki', online: true },
		{ name: 'Drek Omara', ship: 'Tengu', online: true }
	];
</script>

{#snippet header(key: SectionKey, title: string, count?: number, action?: import('svelte').Snippet)}
	<div class="section-header-row">
		<button
			type="button"
			class="section-header"
			aria-expanded={open[key]}
			onclick={() => toggle(key)}
			disabled={locked}
		>
			<svg class="chevron" class:open={open[key]} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
				<path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
			</svg>
			<span class="section-title">{title}</span>
			{#if count !== undefined}<span class="section-count">{count}</span>{/if}
		</button>
		{#if action}{@render action()}{/if}
	</div>
{/snippet}

<div class="sections">
	<!-- System Intel -->
	<section class="sidebar-section">
		{@render header('intel', m.map_proto_section_system_intel())}
		{#if open.intel}
			<div class="section-body sys-intel">
				{#if selected}
					<div class="intel-name-row">
						<span class="intel-name">{selected.name}</span>
						<span class="class-pill" data-class={selected.class}>{selected.class}</span>
					</div>
					<div class="intel-stats">
						<span class="stat-label">{m.map_proto_intel_security()}</span>
						<span class="stat-value">—</span>
						<span class="stat-label">{m.map_proto_intel_statics()}</span>
						<span class="stat-value">
							{#if selected.statics.length}
								<span class="statics">
									<!-- Destination class only (HS/LS/C5…); the wormhole-type code is
									     kept in the model but not surfaced yet. Key by index (a system
									     can have two statics to the same destination). -->
									{#each selected.statics as s, i (i)}<span class="static-badge">{s.dest}</span>{/each}
								</span>
							{:else}—{/if}
						</span>
					</div>
				{:else}
					<p class="empty-note">—</p>
				{/if}
			</div>
		{/if}
	</section>

	<!-- Signatures (bound to the selected system; add / edit / delete). -->
	<section class="sidebar-section">
		{#snippet addSig()}
			{#if selected}
				<button
					type="button"
					class="add-btn"
					title={m.map_proto_sig_add()}
					aria-label={m.map_proto_sig_add()}
					disabled={locked}
					onclick={openAddSig}
				>
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
						<path stroke-linecap="round" d="M12 5v14M5 12h14" />
					</svg>
				</button>
			{/if}
		{/snippet}
		{@render header('signatures', m.map_proto_section_signatures(), scans.length, addSig)}
		{#if open.signatures}
			<div class="section-body">
				{#if scans.length}
					<table class="sig-table">
						<thead><tr>
							<th>{m.map_proto_sig_col_id()}</th>
							<th>{m.map_proto_sig_col_type()}</th>
							<th>{m.map_proto_sig_col_info()}</th>
							<th class="right">{m.map_proto_sig_col_updated()}</th>
						</tr></thead>
						<tbody>
							{#each scans as s (s.sig_id)}
								<!-- Double-click → straight to Edit (the common action); right-click
								     → the Edit / Delete menu. -->
								<tr
									title={provenance(s)}
									class:wormhole={isWormhole(s)}
									ondblclick={() => openEditSig(s)}
									oncontextmenu={(e) => openSigMenu(s, e)}
								>
									<td class="sig-id">{s.sig_id}</td>
									<td><span class="sig-group">{scanType(s)}</span></td>
									<td class="sig-info">{scanInfo(s)}</td>
									<td class="sig-when">{relativeTime(s.updated_at)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{:else}
					<p class="empty-note">
						{selected ? m.map_proto_sig_empty() : m.map_proto_no_selection()}
					</p>
				{/if}
			</div>
		{/if}
	</section>

	<!-- Pilots (sample) -->
	<section class="sidebar-section">
		{@render header('pilots', m.map_proto_section_pilots(), pilots.length)}
		{#if open.pilots}
			<div class="section-body pilots">
				{#each pilots as p (p.name)}
					<div class="pilot-row">
						<span class="pilot-dot" class:online={p.online}></span>
						<span class="pilot-name">{p.name}</span>
						<span class="pilot-ship">{p.ship}</span>
					</div>
				{/each}
			</div>
		{/if}
	</section>

	<!-- Structures (bound to the selected system) -->
	<section class="sidebar-section">
		{@render header('structures', m.map_proto_section_structures(), structures.length)}
		{#if open.structures}
			<div class="section-body structures">
				{#if structures.length}
					{#each structures as st (st.id)}
						<div class="struct-row" title={provenance(st)}>
							<div class="struct-name">{st.name}</div>
							<div class="struct-meta">
								{#if st.hull}{st.hull}{/if}{#if st.hull && st.owner} · {/if}{#if st.owner}{st.owner}{/if}
								<span class="struct-source">{sourceLabel(st.source)}</span>
								{#if st.timer}
									<span class="struct-timer">{timerLabel(st.timer.state)}</span>
								{/if}
							</div>
						</div>
					{/each}
				{:else}
					<p class="empty-note">
						{selected ? m.map_proto_struct_empty() : m.map_proto_no_selection()}
					</p>
				{/if}
			</div>
		{/if}
	</section>

	<!-- Map Canvas Tweaks (prototype ACTIONS). Sits just above the legend, away from
	     the real map-data sections (intel / sigs / pilots / structures). Display
	     PREFERENCES (thickness, label toggles, layout style + auto) live in the cog →
	     Map Preferences dialog now; this keeps only one-shot actions + the throwaway
	     colour-blind A/B switch (removed at promotion). -->
	<section class="sidebar-section">
		{@render header('tweaks', m.map_proto_section_tweaks())}
		{#if open.tweaks}
			<div class="section-body tweaks">
				<button type="button" class="ctl-btn" onclick={onReceiveUpdate}>
					{m.map_proto_receive_update()}
				</button>
				<button type="button" class="ctl-btn" onclick={onRedoLayout}>
					{m.map_proto_layout_redo()}
				</button>
				<label class="toggle">
					<input type="checkbox" bind:checked={colourblind} />
					<span>{m.map_proto_colourblind_palette()}</span>
				</label>
			</div>
		{/if}
	</section>

	<p class="placeholder-note">{m.map_proto_placeholder_note()}</p>
</div>

<!-- Signature context menu (right-click) + the add/edit dialog. Rendered once,
     driven by the per-row handlers above. -->
{#if sigMenu}
	<SigRowMenu
		x={sigMenu.x}
		y={sigMenu.y}
		onEdit={() => {
			const s = sigMenu!.scan;
			sigMenu = null;
			openEditSig(s);
		}}
		onDelete={deleteSig}
		onClose={() => (sigMenu = null)}
	/>
{/if}

<SigEditDialog
	bind:open={dialogOpen}
	mode={dialogMode}
	scan={dialogScan}
	existingIds={existingSigIds}
	onSave={saveSig}
/>

<!-- Delete is not wired to real removal yet — a small "not implemented" notice. -->
<Modal open={deleteNoticeOpen} onClose={() => (deleteNoticeOpen = false)} size="small">
	{#snippet title()}{m.map_proto_sig_delete_title()}{/snippet}
	<p class="notice">{m.map_proto_sig_delete_not_impl()}</p>
	<DialogActions>
		<button type="button" class="btn primary" onclick={() => (deleteNoticeOpen = false)}>
			{m.map_proto_dialog_close()}
		</button>
	</DialogActions>
</Modal>

<style>
	.sections {
		display: flex;
		flex-direction: column;
	}
	.sidebar-section {
		border-bottom: 1px solid var(--space-700);
	}
	.section-header-row {
		display: flex;
		align-items: center;
	}
	.section-header {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 1;
		padding: 8px 12px;
		background: none;
		border: none;
		text-align: left;
		font-size: 10px;
		font-weight: 500;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--slate-400);
		cursor: pointer;
	}
	/* The Signatures "+" add control, sitting at the right of the header row. */
	.add-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		margin-right: 8px;
		padding: 0;
		flex: none;
		background: none;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-400);
		cursor: pointer;
	}
	.add-btn svg {
		width: 12px;
		height: 12px;
	}
	.add-btn:hover:not(:disabled) {
		color: var(--sky);
		border-color: var(--sky);
	}
	.add-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.add-btn:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 1px;
	}
	.section-header:hover {
		color: var(--slate-300);
	}
	.section-header:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}
	.chevron {
		width: 10px;
		height: 10px;
		flex: none;
		transition: transform 0.18s ease;
	}
	.chevron.open {
		transform: rotate(90deg);
	}
	.section-title {
		flex: 1;
	}
	.section-count {
		color: var(--slate-600);
	}

	.section-body {
		font-size: 11px;
	}

	/* System intel */
	.sys-intel {
		padding: 4px 12px 12px;
	}
	.intel-name-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 8px;
	}
	.intel-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--slate-100);
	}
	.class-pill {
		padding: 2px 8px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 700;
		color: var(--c2);
		border: 1px solid var(--c2);
	}
	.class-pill[data-class='C1'] { color: var(--c1); border-color: var(--c1); }
	.class-pill[data-class='C2'] { color: var(--c2); border-color: var(--c2); }
	.class-pill[data-class='C3'] { color: var(--c3); border-color: var(--c3); }
	.class-pill[data-class='C4'] { color: var(--c4); border-color: var(--c4); }
	.class-pill[data-class='C5'] { color: var(--c5); border-color: var(--c5); }
	.class-pill[data-class='C6'] { color: var(--c6); border-color: var(--c6); }
	.class-pill[data-class='HS'] { color: var(--hs); border-color: var(--hs); }
	.class-pill[data-class='LS'] { color: var(--ls); border-color: var(--ls); }
	.class-pill[data-class='NS'] { color: var(--ns); border-color: var(--ns); }
	.class-pill[data-class='P'] { color: var(--pochven); border-color: var(--pochven); }
	.class-pill[data-class='D'] { color: var(--drifter); border-color: var(--drifter); }
	.intel-stats {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 2px 16px;
	}
	.stat-label {
		color: var(--slate-500);
	}
	.stat-value {
		color: var(--slate-300);
	}
	.statics {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}
	.static-badge {
		padding: 1px 5px;
		background: var(--space-800);
		border: 1px solid var(--space-600);
		border-radius: 3px;
		font-size: 10px;
		color: var(--slate-400);
	}
	.empty-note {
		margin: 0;
		color: var(--slate-600);
		font-style: italic;
	}

	/* Signatures table */
	.sig-table {
		width: 100%;
		border-collapse: collapse;
	}
	.sig-table th {
		padding: 4px 8px;
		text-align: left;
		font-weight: 500;
		color: var(--slate-600);
		white-space: nowrap;
	}
	.sig-table th.right {
		text-align: right;
	}
	.sig-table td {
		padding: 5px 8px;
		border-bottom: 1px solid var(--space-800);
	}
	/* Rows are interactive (double-click → edit, right-click → menu); highlight the
	   row under the pointer so that affordance reads. */
	.sig-table tbody tr {
		cursor: pointer;
	}
	.sig-table tbody tr:hover {
		background: var(--space-800);
	}
	.sig-id {
		color: var(--slate-400);
	}
	.sig-group {
		font-weight: 600;
		color: var(--slate-300);
	}
	/* Wormhole sigs are the headline content of a wormholers' tool — give the whole
	   row a distinct sky tint so holes stand out from cosmic (data/relic/gas/ore)
	   sites at a glance. */
	.sig-table tr.wormhole .sig-group {
		color: var(--sky);
	}
	.sig-table tr.wormhole .sig-info {
		color: var(--sky);
	}
	.sig-info {
		max-width: 90px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--slate-400);
	}
	.sig-when {
		text-align: right;
		color: var(--slate-600);
		white-space: nowrap;
	}

	/* Pilots */
	.pilots {
		padding: 4px 0 8px;
	}
	.pilot-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 12px;
	}
	.pilot-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--slate-600);
		flex: none;
	}
	.pilot-dot.online {
		background: var(--emerald);
	}
	.pilot-name {
		flex: 1;
		color: var(--slate-300);
	}
	.pilot-ship {
		color: var(--slate-500);
	}

	/* Structures */
	.structures {
		padding: 4px 12px 10px;
	}
	.struct-row + .struct-row {
		margin-top: 8px;
		padding-top: 8px;
		border-top: 1px solid var(--space-800);
	}
	.struct-name {
		color: var(--slate-300);
		margin-bottom: 2px;
	}
	.struct-meta {
		color: var(--slate-500);
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
	}
	.struct-source {
		padding: 0 5px;
		border: 1px solid var(--space-600);
		border-radius: 3px;
		font-size: 10px;
		color: var(--slate-400);
	}
	.struct-timer {
		padding: 0 5px;
		border-radius: 3px;
		font-size: 10px;
		color: var(--alert-danger);
		border: 1px solid var(--alert-danger);
	}

	/* Tweaks */
	.tweaks {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.6rem 12px 0.7rem;
		color: var(--slate-200);
	}
	.ctl-btn {
		padding: 0.35rem 0.6rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.7rem;
		text-align: left;
		cursor: pointer;
	}
	.ctl-btn:focus-visible,
	.tweaks input:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.toggle {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		font-size: 0.7rem;
		cursor: pointer;
	}
	.toggle input {
		accent-color: var(--sky);
		cursor: pointer;
	}

	.placeholder-note {
		margin: 0;
		padding: 8px 12px;
		font-size: 10px;
		font-style: italic;
		color: var(--slate-600);
	}

	/* The delete "not implemented" notice body (in the Modal). */
	.notice {
		margin: 0;
		font-size: 0.8rem;
		line-height: 1.5;
		color: var(--slate-300);
	}
</style>
