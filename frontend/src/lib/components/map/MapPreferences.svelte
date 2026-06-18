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
	import type { LayoutDirection } from '$lib/map/types';

	// Live edits preview on the canvas behind the (blurred) dialog. So Cancel must
	// REVERT to where things stood when the dialog opened — snapshot the values on
	// open, and write them back on cancel. OK simply closes, keeping the changes.
	type Snapshot = {
		thickness: number;
		showMass: boolean;
		showWhType: boolean;
		showSignatures: boolean;
		showDirection: boolean;
		layoutDir: LayoutDirection;
		autoLayout: boolean;
	};

	let {
		open = $bindable(),
		thickness = $bindable(),
		thicknessMin,
		thicknessMax,
		showMass = $bindable(),
		showWhType = $bindable(),
		showSignatures = $bindable(),
		showDirection = $bindable(),
		layoutDir,
		autoLayout = $bindable(),
		onSelectLayout
	}: {
		open: boolean;
		thickness: number;
		thicknessMin: number;
		thicknessMax: number;
		showMass: boolean;
		showWhType: boolean;
		showSignatures: boolean;
		showDirection: boolean;
		/** The currently-selected layout style (highlights the segmented control). */
		layoutDir: LayoutDirection;
		/** When ON, a map change reflows the whole map in the selected style. */
		autoLayout: boolean;
		/** Set the selected layout style (the parent reflows immediately if auto is on). */
		onSelectLayout: (dir: LayoutDirection) => void;
	} = $props();

	// The four layout styles, in segmented-control order, with their labels.
	const layoutStyles: { dir: LayoutDirection; label: () => string }[] = [
		{ dir: 'LR', label: m.map_proto_layout_lr },
		{ dir: 'RL', label: m.map_proto_layout_rl },
		{ dir: 'TB', label: m.map_proto_layout_tb },
		{ dir: 'BT', label: m.map_proto_layout_bt }
	];

	// Snapshot the live values whenever the dialog transitions to open, so Cancel can
	// restore them. `layoutDir` is not a bindable here (it is owned by the parent via
	// onSelectLayout), so it is reverted through that callback.
	let snapshot: Snapshot | null = null;
	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			snapshot = {
				thickness,
				showMass,
				showWhType,
				showSignatures,
				showDirection,
				layoutDir,
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
			showMass = snapshot.showMass;
			showWhType = snapshot.showWhType;
			showSignatures = snapshot.showSignatures;
			showDirection = snapshot.showDirection;
			autoLayout = snapshot.autoLayout;
			if (layoutDir !== snapshot.layoutDir) onSelectLayout(snapshot.layoutDir);
		}
		open = false;
	}
</script>

<Modal {open} onClose={cancel} backdrop="blur">
	{#snippet title()}{m.map_proto_prefs_title()}{/snippet}

	<div class="prefs">
		<div class="layout-control">
			<span class="layout-label" id="prefs-layout-style-label">
				{m.map_proto_layout_heading()}
			</span>
			<div class="layout-segmented" role="group" aria-labelledby="prefs-layout-style-label">
				{#each layoutStyles as style (style.dir)}
					<button
						type="button"
						class="seg-btn"
						aria-pressed={layoutDir === style.dir}
						onclick={() => onSelectLayout(style.dir)}
					>
						{style.label()}
					</button>
				{/each}
			</div>

			<label class="toggle">
				<input type="checkbox" bind:checked={autoLayout} />
				<span>{m.map_proto_layout_auto()}</span>
			</label>
		</div>

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
			<input type="checkbox" bind:checked={showSignatures} />
			<span>{m.map_proto_show_signatures()}</span>
		</label>
		<label class="toggle">
			<input type="checkbox" bind:checked={showDirection} />
			<span>{m.map_proto_show_direction()}</span>
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
	.seg-btn:focus-visible,
	.prefs input:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
	/* Layout control: a label, a 4-way segmented style picker, the auto toggle. */
	.layout-control {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding: 0.5rem;
		background: var(--space-950);
		border: 1px solid var(--space-700);
		border-radius: 4px;
	}
	.layout-label {
		font-size: 0.75rem;
		font-weight: 700;
		color: var(--slate-200);
	}
	.layout-segmented {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.25rem;
	}
	.seg-btn {
		padding: 0.35rem 0.4rem;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-300);
		font: inherit;
		font-size: 0.75rem;
		text-align: center;
		cursor: pointer;
	}
	/* The selected style: sky border + tint, brighter text. aria-pressed is the
	   source of truth (not colour alone) — the pressed state is exposed to AT. */
	.seg-btn[aria-pressed='true'] {
		border-color: var(--sky);
		background: var(--space-700);
		color: var(--slate-100);
		font-weight: 700;
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
</style>
