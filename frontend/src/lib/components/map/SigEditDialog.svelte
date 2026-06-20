<script lang="ts">
	// Add / edit a signature, on the map sidebar. Add and edit are the SAME form
	// (sig id + site type + name); the only differences are the title, whether the
	// fields seed from an existing scan, and the wormhole case. PROTOTYPE: cosmic
	// sites (data / relic / gas / ore / unknown) are editable here; WORMHOLE sigs show
	// a gated notice — their type / mass / lifetime / "leads to" belong to the
	// connection propagation group, a later piece of work.
	//
	// Built on the shared Modal + DialogActions parts. Like MapPreferences, edits are
	// drafted locally and only committed on save, so Cancel / Escape / backdrop discard.
	import Modal from '$lib/components/Modal.svelte';
	import DialogActions from '$lib/components/DialogActions.svelte';
	import type { ScanResult } from '$lib/map/types';
	import { m } from '$lib/paraglide/messages';

	let {
		open = $bindable(),
		mode,
		scan = null,
		existingIds,
		onSave
	}: {
		open: boolean;
		/** `add` builds a fresh ScanResult; `edit` mutates an existing one. */
		mode: 'add' | 'edit';
		/** The scan being edited (edit mode); `null` for add. */
		scan?: ScanResult | null;
		/** All sig ids already in the system — for the uniqueness check. In edit mode
		 *  the edited scan's own id is excluded by the caller so a no-op rename passes. */
		existingIds: string[];
		/** Commit: `name` is null when blank. Caller writes it to session state. */
		onSave: (fields: { sig_id: string; site_type: string | null; name: string | null }) => void;
	} = $props();

	// The cosmic site types this form offers. Wormhole is deliberately absent — typed
	// wormholes come in scanned, and editing them is gated (see below). `null` =
	// Unknown (unclassified).
	const SITE_TYPES: { value: string | null; label: () => string }[] = [
		{ value: null, label: () => m.map_proto_sig_type_unknown() },
		{ value: 'Data Site', label: () => m.map_proto_sig_type_data() },
		{ value: 'Relic Site', label: () => m.map_proto_sig_type_relic() },
		{ value: 'Gas Site', label: () => m.map_proto_sig_type_gas() },
		{ value: 'Ore Site', label: () => m.map_proto_sig_type_ore() }
	];

	// A wormhole sig is the gated case: show a notice instead of fields.
	const isWormhole = $derived(mode === 'edit' && scan?.site_type === 'Wormhole');

	// Draft fields. `siteTypeIdx` indexes SITE_TYPES (a <select> binds cleanly to an
	// index; null site_type isn't a usable option value).
	let sigId = $state('');
	let siteTypeIdx = $state(0);
	let name = $state('');
	let error = $state<string | null>(null);

	// Seed the draft whenever the dialog transitions to open.
	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			error = null;
			if (mode === 'edit' && scan) {
				sigId = scan.sig_id;
				const idx = SITE_TYPES.findIndex((t) => t.value === scan.site_type);
				siteTypeIdx = idx >= 0 ? idx : 0;
				name = scan.name ?? '';
			} else {
				sigId = '';
				siteTypeIdx = 0;
				name = '';
			}
		}
		wasOpen = open;
	});

	function save() {
		const id = sigId.trim();
		if (!id) {
			error = m.map_proto_sig_id_required();
			return;
		}
		// Sig ids are case-insensitively unique within a system (ABC-123).
		if (existingIds.some((e) => e.toLowerCase() === id.toLowerCase())) {
			error = m.map_proto_sig_dup_id();
			return;
		}
		const trimmedName = name.trim();
		onSave({
			sig_id: id,
			site_type: SITE_TYPES[siteTypeIdx].value,
			name: trimmedName === '' ? null : trimmedName
		});
		open = false;
	}

	function cancel() {
		open = false;
	}
</script>

<Modal {open} onClose={cancel} size="small">
	{#snippet title()}
		{mode === 'add' ? m.map_proto_sig_add_title() : m.map_proto_sig_edit_title()}
	{/snippet}

	{#if isWormhole}
		<p class="gated">{m.map_proto_sig_edit_wh_gated()}</p>
		<DialogActions>
			<button type="button" class="btn primary" onclick={cancel}>
				{m.map_proto_dialog_close()}
			</button>
		</DialogActions>
	{:else}
		<form
			class="sig-form"
			onsubmit={(e) => {
				e.preventDefault();
				save();
			}}
		>
			<label class="field">
				<span>{m.map_proto_sig_field_id()}</span>
				<input
					type="text"
					bind:value={sigId}
					placeholder="ABC-123"
					oninput={() => (error = null)}
				/>
			</label>
			<label class="field">
				<span>{m.map_proto_sig_field_type()}</span>
				<select bind:value={siteTypeIdx}>
					{#each SITE_TYPES as t, i (i)}
						<option value={i}>{t.label()}</option>
					{/each}
				</select>
			</label>
			<label class="field">
				<span>{m.map_proto_sig_field_name()}</span>
				<input type="text" bind:value={name} />
			</label>

			{#if error}<p class="error" role="alert">{error}</p>{/if}

			<DialogActions>
				<button type="button" class="btn ghost" onclick={cancel}>{m.dialog_cancel()}</button>
				<button type="submit" class="btn primary">{m.dialog_ok()}</button>
			</DialogActions>
		</form>
	{/if}
</Modal>

<style>
	.sig-form {
		display: flex;
		flex-direction: column;
		gap: 0.7rem;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		font-size: 0.8rem;
		color: var(--slate-200);
	}
	.field input,
	.field select {
		padding: 0.4rem 0.5rem;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--slate-100);
		font: inherit;
		font-size: 0.8rem;
	}
	.field input:focus-visible,
	.field select:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 1px;
	}
	.error {
		margin: 0;
		font-size: 0.75rem;
		color: var(--alert-danger);
	}
	.gated {
		margin: 0;
		font-size: 0.8rem;
		line-height: 1.5;
		color: var(--slate-300);
	}
</style>
