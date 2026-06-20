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

	// Live edits preview on the canvas behind the (blurred) dialog. So Cancel must
	// REVERT to where things stood when the dialog opened — snapshot the values on
	// open, and write them back on cancel. OK simply closes, keeping the changes.
	// (The layout STYLE picker + "apply now" moved out to the tab-bar split-button;
	// only "auto-layout on changes" remains here, alongside the other toggles.)
	type Snapshot = {
		thickness: number;
		nodeSpacing: number;
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
		nodeSpacing = $bindable(),
		spacingMin,
		spacingMax,
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
		/** Cross-axis layout spacing, as a percent multiplier (100 = compact base).
		 *  Changing it reflows the active tab so a busy chain spreads apart. */
		nodeSpacing: number;
		spacingMin: number;
		spacingMax: number;
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
				nodeSpacing,
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
			nodeSpacing = snapshot.nodeSpacing;
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

		<label class="thickness">
			<span class="row">
				<span>{m.map_proto_node_spacing()}</span>
				<output class="thickness-value">{nodeSpacing}%</output>
			</span>
			<input
				type="range"
				min={spacingMin}
				max={spacingMax}
				step="10"
				bind:value={nodeSpacing}
				aria-label={m.map_proto_node_spacing()}
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
