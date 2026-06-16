<script lang="ts">
	// The map sidebar: collapsible intel sections (System Intel / Signatures /
	// Pilots / Structures) modelled on the wireframe, plus a "Map Canvas Tweaks"
	// section holding the prototype display controls. The intel sections render
	// SAMPLE data for now — they're wired to the chain-map model on the backend
	// track. Lives under components/map/ as part of the canvas theme seam.
	import { m } from '$lib/paraglide/messages';
	import type { System, LayoutDirection } from '$lib/map/types';

	let {
		selected,
		thickness = $bindable(),
		thicknessMin,
		thicknessMax,
		showMass = $bindable(),
		showWhType = $bindable(),
		showDirection = $bindable(),
		layoutOpen = $bindable(),
		onRedoLayout,
		onReceiveUpdate
	}: {
		/** The system the intel sections describe (a stand-in for canvas selection). */
		selected: System | null;
		thickness: number;
		thicknessMin: number;
		thicknessMax: number;
		showMass: boolean;
		showWhType: boolean;
		showDirection: boolean;
		layoutOpen: boolean;
		onRedoLayout: (dir: LayoutDirection) => void;
		onReceiveUpdate: () => void;
	} = $props();

	// Per-section open state. Sample sections start open (as in the wireframe).
	let open = $state({
		intel: true,
		signatures: true,
		pilots: true,
		structures: true,
		tweaks: true
	});
	type SectionKey = keyof typeof open;
	function toggle(k: SectionKey) {
		open[k] = !open[k];
	}

	// Sample data for the placeholder sections (replaced by real chain-map data).
	const sigs = [
		{ id: 'ABC-123', group: 'WH', colour: 'var(--c3)', info: 'C3a → J234567', when: '2m' },
		{ id: 'DEF-456', group: 'WH', colour: 'var(--hs)', info: 'HSa → Jita', when: '5m' },
		{ id: 'GHI-789', group: 'REL', colour: 'var(--amber)', info: 'Unknown', when: '12m' },
		{ id: 'JKL-012', group: 'DAT', colour: 'var(--sky)', info: 'Unknown', when: '1h' }
	];
	const pilots = [
		{ name: 'Alara Voss', ship: 'Loki', online: true },
		{ name: 'Drek Omara', ship: 'Tengu', online: true }
	];
</script>

{#snippet header(key: SectionKey, title: string, count?: number)}
	<button
		type="button"
		class="section-header"
		aria-expanded={open[key]}
		onclick={() => toggle(key)}
	>
		<svg class="chevron" class:open={open[key]} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
			<path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" />
		</svg>
		<span class="section-title">{title}</span>
		{#if count !== undefined}<span class="section-count">{count}</span>{/if}
	</button>
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
									{#each selected.statics as s (s.code)}<span class="static-badge">{s.code}</span>{/each}
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

	<!-- Signatures (sample) -->
	<section class="sidebar-section">
		{@render header('signatures', m.map_proto_section_signatures(), sigs.length)}
		{#if open.signatures}
			<div class="section-body">
				<table class="sig-table">
					<thead><tr><th>ID</th><th>Type</th><th>Info</th><th class="right">When</th></tr></thead>
					<tbody>
						{#each sigs as s (s.id)}
							<tr>
								<td class="sig-id">{s.id}</td>
								<td><span class="sig-group" style:color={s.colour}>{s.group}</span></td>
								<td class="sig-info">{s.info}</td>
								<td class="sig-when">{s.when}</td>
							</tr>
						{/each}
					</tbody>
				</table>
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

	<!-- Structures (sample) -->
	<section class="sidebar-section">
		{@render header('structures', m.map_proto_section_structures(), 1)}
		{#if open.structures}
			<div class="section-body structures">
				<div class="struct-name">Fort Nightfall</div>
				<div class="struct-meta">Fortizar · Brave Collective</div>
			</div>
		{/if}
	</section>

	<!-- Map Canvas Tweaks (prototype display controls) -->
	<section class="sidebar-section">
		{@render header('tweaks', m.map_proto_section_tweaks())}
		{#if open.tweaks}
			<div class="section-body tweaks">
				<button type="button" class="ctl-btn" onclick={onReceiveUpdate}>
					{m.map_proto_receive_update()}
				</button>

				<button
					type="button"
					class="ctl-btn"
					aria-expanded={layoutOpen}
					onclick={() => (layoutOpen = !layoutOpen)}
				>
					{m.map_proto_layout_heading()}
				</button>
				{#if layoutOpen}
					<div class="layout-options" role="group" aria-label={m.map_proto_layout_toggle()}>
						<button type="button" onclick={() => onRedoLayout('LR')}>{m.map_proto_layout_lr()}</button>
						<button type="button" onclick={() => onRedoLayout('RL')}>{m.map_proto_layout_rl()}</button>
						<button type="button" onclick={() => onRedoLayout('TB')}>{m.map_proto_layout_tb()}</button>
						<button type="button" onclick={() => onRedoLayout('BT')}>{m.map_proto_layout_bt()}</button>
						<button type="button" onclick={() => onRedoLayout('radial')}>{m.map_proto_layout_radial()}</button>
					</div>
				{/if}

				<label class="thickness">
					<span class="row">
						<span>{m.map_proto_edge_thickness()}</span>
						<output class="thickness-value">{thickness}</output>
					</span>
					<input
						type="range"
						min={thicknessMin}
						max={thicknessMax}
						step="1"
						bind:value={thickness}
						aria-label={m.map_proto_edge_thickness()}
					/>
				</label>
				<label class="toggle">
					<input type="checkbox" bind:checked={showMass} />
					<span>{m.map_proto_show_mass_labels()}</span>
				</label>
				<label class="toggle">
					<input type="checkbox" bind:checked={showWhType} />
					<span>{m.map_proto_show_whtype_labels()}</span>
				</label>
				<label class="toggle">
					<input type="checkbox" bind:checked={showDirection} />
					<span>{m.map_proto_show_direction()}</span>
				</label>
			</div>
		{/if}
	</section>

	<p class="placeholder-note">{m.map_proto_placeholder_note()}</p>
</div>

<style>
	.sections {
		display: flex;
		flex-direction: column;
	}
	.sidebar-section {
		border-bottom: 1px solid var(--space-700);
	}
	.section-header {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
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
	.sig-id {
		color: var(--slate-400);
	}
	.sig-group {
		font-weight: 600;
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
	.struct-name {
		color: var(--slate-300);
		margin-bottom: 2px;
	}
	.struct-meta {
		color: var(--slate-500);
	}

	/* Tweaks */
	.tweaks {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		padding: 0.6rem 12px 0.7rem;
		color: var(--slate-200);
	}
	.ctl-btn,
	.layout-options button {
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
	.layout-options button:focus-visible,
	.tweaks input:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.layout-options {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		padding: 0.3rem;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
	}
	.thickness {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
		font-size: 0.7rem;
	}
	.thickness .row {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.thickness-value {
		min-width: 1.5em;
		padding: 0 0.3rem;
		text-align: center;
		background: var(--space-800);
		border-radius: 3px;
		font-weight: 700;
		color: var(--slate-100);
	}
	.thickness input[type='range'] {
		width: 100%;
		accent-color: var(--sky);
		cursor: pointer;
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
</style>
