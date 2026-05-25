<!--
	/preferences — accessibility settings.

	Reachable anonymously (settings work before/without login; see +layout.server
	public-route list). Changes are STAGED: selecting any control previews it live
	on <html> but does not persist. While the staged set differs from the persisted
	set, Apply (persist the batch) and Discard (revert) appear. Returning every
	control to its persisted value returns to the clean state. Navigating away while
	dirty silently discards the previews so <html> never disagrees with persistence.
	A Reset-to-defaults control is always available as the recovery surface.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import PreferenceControl from '$lib/components/PreferenceControl.svelte';
	import { preferences } from '$lib/preferences/store.svelte';
	import {
		DEFAULT_PREFERENCES,
		type PreferenceKey,
		type Preferences
	} from '$lib/preferences/schema';

	// The staged set the controls bind to. Initialised from the persisted set once
	// the store has hydrated, and re-synced to the persisted baseline after every
	// Apply / Discard / Reset so `dirty` returns to false.
	let staged = $state<Preferences>({ ...preferences.persisted });

	// Keep `staged` in sync with persistence on first hydrate / external changes
	// (e.g. login reconciliation), but only while the user has nothing staged —
	// never clobber an in-progress edit.
	$effect(() => {
		const persisted = preferences.persisted;
		if (!dirty) staged = { ...persisted };
	});

	const dirty = $derived(
		(Object.keys(DEFAULT_PREFERENCES) as PreferenceKey[]).some(
			(k) => staged[k] !== preferences.persisted[k]
		)
	);

	function select(key: PreferenceKey, value: string) {
		staged = { ...staged, [key]: value };
		// Live preview only — persistence happens on Apply.
		preferences.preview(staged);
	}

	async function apply() {
		await preferences.commit(staged);
		staged = { ...preferences.persisted };
	}

	function discard() {
		preferences.revertToPersisted();
		staged = { ...preferences.persisted };
	}

	async function reset() {
		await preferences.resetToDefaults();
		staged = { ...preferences.persisted };
	}

	// Leaving with unapplied changes silently discards the previews so the next
	// page reflects persisted state (in-app nav + a teardown backstop). A hard
	// reload / tab close persists nothing, so it naturally shows persisted values.
	beforeNavigate(() => {
		if (dirty) preferences.revertToPersisted();
	});
	onDestroy(() => {
		if (dirty) preferences.revertToPersisted();
	});

	const textSizeOptions = [
		{ value: 'auto', label: 'Auto' },
		{ value: 'small', label: 'Small' },
		{ value: 'regular', label: 'Regular' },
		{ value: 'large', label: 'Large' }
	];
	const triStateOptions = [
		{ value: 'auto', label: 'Auto (follow system)' },
		{ value: 'on', label: 'On' },
		{ value: 'off', label: 'Off' }
	];
	const toggleOptions = [
		{ value: 'off', label: 'Off' },
		{ value: 'on', label: 'On' }
	];
</script>

<svelte:head><title>Preferences · E-R Bridge</title></svelte:head>

<main class="preferences">
	<h1>Accessibility preferences</h1>
	<p class="intro">
		Changes preview as you make them. Click <strong>Apply</strong> to save them — when
		you're signed in they're stored on your account and follow you across devices.
	</p>

	<PreferenceControl
		label="Text size"
		description="Scales all text in the interface. Auto follows your browser's default size."
		value={staged.text_size}
		options={textSizeOptions}
		onSelect={(v) => select('text_size', v)}
	/>

	<PreferenceControl
		label="Reduce motion"
		description="Disables animations and transitions. Auto follows your system setting."
		value={staged.reduce_motion}
		options={triStateOptions}
		onSelect={(v) => select('reduce_motion', v)}
	/>

	<PreferenceControl
		label="High contrast"
		description="Increases contrast between text and background. Auto follows your system setting."
		value={staged.high_contrast}
		options={triStateOptions}
		onSelect={(v) => select('high_contrast', v)}
	/>

	<PreferenceControl
		label="Larger interactive targets"
		description="Increases the minimum size of buttons and links to make them easier to hit."
		value={staged.large_targets}
		options={toggleOptions}
		onSelect={(v) => select('large_targets', v)}
	/>

	<PreferenceControl
		label="Dyslexia-friendly typeface"
		description="Replaces the interface font with Atkinson Hyperlegible."
		value={staged.dyslexia_font}
		options={toggleOptions}
		onSelect={(v) => select('dyslexia_font', v)}
	/>

	<div class="actions">
		{#if dirty}
			<button type="button" class="btn apply" onclick={apply}>Apply</button>
			<button type="button" class="btn discard" onclick={discard}>Discard</button>
		{/if}
		<button type="button" class="btn reset" onclick={reset}>Reset to defaults</button>
	</div>
</main>

<style>
	.preferences {
		/* Own scroll region inside the fixed-height app shell — see /about for the
		   rationale. flex:1 + min-height:0 fill remaining height and allow shrink;
		   the 640px column stays centred via max-width + auto margins. */
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		width: 100%;
		max-width: 640px;
		margin: 0 auto;
		padding: 32px 24px 64px;
	}

	h1 {
		margin: 0 0 8px;
		font-size: 1.25rem;
		font-weight: 600;
		color: var(--slate-100);
	}

	.intro {
		margin: 0 0 16px;
		font-size: 0.8125rem;
		line-height: 1.55;
		color: var(--slate-400);
	}

	.actions {
		display: flex;
		gap: 12px;
		margin-top: 24px;
	}

	/* Fixed px sizing and a hard-coded palette (NOT the design tokens that
	   high_contrast / text_size override) so these controls stay usable under any
	   applied preview — the same constraint the deleted revert bar carried. They
	   are the recovery surface, so they must never be broken by a previewed change. */
	.btn {
		padding: 10px 18px;
		font-family: ui-sans-serif, system-ui, sans-serif;
		font-size: 14px;
		line-height: 1;
		min-height: 40px;
		border-radius: 6px;
		border: 1px solid #ffffff;
		cursor: pointer;
	}
	.btn:focus-visible {
		outline: 2px solid #ffffff;
		outline-offset: 2px;
	}

	.apply {
		background: #38bdf8;
		border-color: #38bdf8;
		color: #05080f;
		font-weight: 600;
	}
	.discard {
		background: transparent;
		color: #ffffff;
	}
	.reset {
		background: transparent;
		color: #ffffff;
		margin-left: auto;
	}
</style>
