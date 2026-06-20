<script lang="ts">
	// Per-user map display preferences, in a dialog opened from the cog on the tab
	// bar. PROTOTYPE: the values are session-only $state bound back to MapCanvas, so
	// edits apply live (the dialog uses a blurred backdrop so the canvas stays visible
	// behind it and the changes show as you make them). Persistence + per-map/default
	// inheritance are deferred to the real-route promotion change — this component is
	// the UI seam for them. Colour-blind palette is NOT here: it is a throwaway A/B
	// switch that lives in the sidebar Tweaks section and is removed at promotion.
	import Modal from '$lib/components/Modal.svelte';
	import DialogActions from '$lib/components/DialogActions.svelte';
	import { m } from '$lib/paraglide/messages';
	import type { LayoutAlgorithm } from '$lib/map/types';

	// Live edits preview on the canvas behind the (blurred) dialog. So Cancel must
	// REVERT to where things stood when the dialog opened — snapshot the values on
	// open, and write them back on cancel. OK simply closes, keeping the changes.
	// (The layout STYLE picker + "apply now" moved out to the tab-bar split-button;
	// only "auto-layout on changes" remains here, alongside the other toggles.)
	type Snapshot = {
		thickness: number;
		xSpacing: number;
		ySpacing: number;
		layoutAlgo: LayoutAlgorithm;
		showMass: boolean;
		showWhType: boolean;
		showSignatures: boolean;
		showDirection: boolean;
		animateDirection: boolean;
		autoLayout: boolean;
	};

	let {
		open = $bindable(),
		thickness = $bindable(),
		thicknessMin,
		thicknessMax,
		xSpacing = $bindable(),
		ySpacing = $bindable(),
		spacingMin,
		spacingMax,
		layoutAlgo = $bindable(),
		showMass = $bindable(),
		showWhType = $bindable(),
		showSignatures = $bindable(),
		showDirection = $bindable(),
		animateDirection = $bindable(),
		autoLayout = $bindable()
	}: {
		open: boolean;
		thickness: number;
		thicknessMin: number;
		thicknessMax: number;
		/** Horizontal (X) screen-axis spacing, as a percent multiplier (100 = compact base):
		 *  ALWAYS spreads nodes left↔right whatever the layout direction (MapCanvas maps it
		 *  to the rank/cross multiplier per direction). Changing it reflows the active tab. */
		xSpacing: number;
		/** Vertical (Y) screen-axis spacing, as a percent multiplier (100 = compact base):
		 *  ALWAYS spreads nodes up↕down whatever the layout direction. Shares the min/max
		 *  with the X slider. Changing it reflows the active tab. */
		ySpacing: number;
		spacingMin: number;
		spacingMax: number;
		/** Which layout ENGINE seeds positions (tidy-tree vs dagre). Changing it reflows
		 *  the active tab. */
		layoutAlgo: LayoutAlgorithm;
		showMass: boolean;
		showWhType: boolean;
		showSignatures: boolean;
		showDirection: boolean;
		/** Taste pref: drift the direction arrow along the line (default off, separate
		 *  from prefers-reduced-motion). Only meaningful while showDirection is on. */
		animateDirection: boolean;
		/** When ON, a map change reflows the whole map in the selected style. */
		autoLayout: boolean;
	} = $props();

	// Snapshot the live values whenever the dialog transitions to open, so Cancel can
	// restore them.
	let snapshot: Snapshot | null = null;
	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			snapshot = {
				thickness,
				xSpacing,
				ySpacing,
				layoutAlgo,
				showMass,
				showWhType,
				showSignatures,
				showDirection,
				animateDirection,
				autoLayout
			};
		}
		wasOpen = open;
	});

	// OK: keep the live changes, just close.
	function confirm() {
		open = false;
	}

	// Cancel / Escape / backdrop: revert to the snapshot, then close. (Mirrors the
	// app's ConfirmDialog, where Escape + backdrop both mean cancel.)
	function cancel() {
		if (snapshot) {
			thickness = snapshot.thickness;
			xSpacing = snapshot.xSpacing;
			ySpacing = snapshot.ySpacing;
			layoutAlgo = snapshot.layoutAlgo;
			showMass = snapshot.showMass;
			showWhType = snapshot.showWhType;
			showSignatures = snapshot.showSignatures;
			showDirection = snapshot.showDirection;
			animateDirection = snapshot.animateDirection;
			autoLayout = snapshot.autoLayout;
		}
		open = false;
	}
</script>

<Modal {open} onClose={cancel} backdrop="blur">
	{#snippet title()}{m.map_proto_prefs_title()}{/snippet}

	<div class="prefs">
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

		<!-- Two independent SCREEN-axis spacing sliders: X always spreads nodes left↔right,
		     Y always up↕down, whatever the layout direction (MapCanvas maps each to the
		     engine's rank/cross multiplier per direction). -->
		<label class="thickness">
			<span class="row">
				<span>{m.map_proto_spacing_horizontal()}</span>
				<output class="thickness-value">{xSpacing}%</output>
			</span>
			<input
				type="range"
				min={spacingMin}
				max={spacingMax}
				step="10"
				bind:value={xSpacing}
				aria-label={m.map_proto_spacing_horizontal()}
			/>
		</label>

		<label class="thickness">
			<span class="row">
				<span>{m.map_proto_spacing_vertical()}</span>
				<output class="thickness-value">{ySpacing}%</output>
			</span>
			<input
				type="range"
				min={spacingMin}
				max={spacingMax}
				step="10"
				bind:value={ySpacing}
				aria-label={m.map_proto_spacing_vertical()}
			/>
		</label>

		<!-- Layout engine: a two-option segmented control. Changing it reflows the active
		     tab (machine-owned). aria-checked on each option carries the state to AT; the
		     fieldset/legend names the group. -->
		<fieldset class="segmented">
			<legend>{m.map_proto_layout_algo()}</legend>
			<div class="segments" role="radiogroup" aria-label={m.map_proto_layout_algo()}>
				<button
					type="button"
					class="segment"
					role="radio"
					aria-checked={layoutAlgo === 'dagre'}
					onclick={() => (layoutAlgo = 'dagre')}
				>
					{m.map_proto_layout_algo_dagre()}
				</button>
				<button
					type="button"
					class="segment"
					role="radio"
					aria-checked={layoutAlgo === 'tidy-tree'}
					onclick={() => (layoutAlgo = 'tidy-tree')}
				>
					{m.map_proto_layout_algo_tidy()}
				</button>
			</div>
		</fieldset>

		<label class="toggle">
			<input type="checkbox" bind:checked={showMass} />
			<span>{m.map_proto_show_mass_labels()}</span>
		</label>
		<label class="toggle">
			<input type="checkbox" bind:checked={showWhType} />
			<span>{m.map_proto_show_whtype_labels()}</span>
		</label>
		<label class="toggle">
			<input type="checkbox" bind:checked={showSignatures} />
			<span>{m.map_proto_show_signatures()}</span>
		</label>
		<label class="toggle">
			<input type="checkbox" bind:checked={showDirection} />
			<span>{m.map_proto_show_direction()}</span>
		</label>
		<!-- Drift animation only matters while the arrow is shown; disable when not. -->
		<label class="toggle" class:disabled={!showDirection}>
			<input type="checkbox" bind:checked={animateDirection} disabled={!showDirection} />
			<span>{m.map_proto_animate_direction()}</span>
		</label>
		<label class="toggle">
			<input type="checkbox" bind:checked={autoLayout} />
			<span>{m.map_proto_layout_auto()}</span>
		</label>

		<!-- Footer (shared DialogActions): Cancel (ghost, reverts the live edits) + OK
		     (primary, keeps them). A value dialog, so both are type="button". -->
		<DialogActions>
			<button type="button" class="btn ghost" onclick={cancel}>{m.dialog_cancel()}</button>
			<button type="button" class="btn primary" onclick={confirm}>{m.dialog_ok()}</button>
		</DialogActions>
	</div>
</Modal>

<style>
	.prefs {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		color: var(--slate-200);
	}
	.prefs input:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	.thickness {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		font-size: 0.8rem;
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
	/* Layout-engine segmented control. The fieldset/legend name the group; the two
	   buttons are a radiogroup, the active one tinted (aria-checked is the source of
	   truth, not colour alone). */
	.segmented {
		margin: 0;
		padding: 0;
		border: 0;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}
	.segmented legend {
		padding: 0;
		font-size: 0.8rem;
		color: var(--slate-200);
	}
	.segments {
		display: flex;
		gap: 0;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		overflow: hidden;
		width: fit-content;
	}
	.segment {
		padding: 4px 12px;
		background: none;
		border: 0;
		color: var(--slate-300);
		font: inherit;
		font-size: 0.75rem;
		cursor: pointer;
	}
	.segment + .segment {
		border-left: 1px solid var(--space-700);
	}
	.segment:hover {
		background: var(--space-800);
		color: var(--slate-100);
	}
	.segment[aria-checked='true'] {
		background: var(--space-700);
		color: var(--slate-100);
		font-weight: 700;
	}
	.segment:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}
	.toggle {
		display: flex;
		align-items: center;
		gap: 0.45rem;
		font-size: 0.8rem;
		cursor: pointer;
	}
	.toggle input {
		accent-color: var(--sky);
		cursor: pointer;
	}
	.toggle.disabled {
		opacity: 0.45;
		cursor: not-allowed;
	}
	.toggle.disabled input {
		cursor: not-allowed;
	}
</style>
