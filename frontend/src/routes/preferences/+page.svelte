<!--
	/preferences — account preferences, organised into tabs.

	Tabs ("General" → locale; "Accessibility" → text/motion/contrast/targets/font)
	are a PRESENTATION layer over a single staged batch: one `staged` set, one
	`dirty` flag, one Apply/Discard/Reset bar across both tabs. Switching tabs
	neither commits, discards, nor resets — it only changes which controls show.

	Reachable anonymously (settings work before/without login; see +layout.server
	public-route list). Changes are STAGED: selecting any control previews it live
	(accessibility prefs on <html>; locale is bridged to Paraglide's cookie only on
	Apply) but does not persist. While the staged set differs from the persisted
	set, Apply (persist the batch) and Discard (revert) appear. Returning every
	control to its persisted value returns to the clean state. Navigating away while
	dirty silently discards the previews so <html> never disagrees with persistence.
	A Reset-to-defaults control is always available as the recovery surface.
-->
<script lang="ts">
	import { onDestroy } from 'svelte';
	import { beforeNavigate } from '$app/navigation';
	import { m } from '$lib/paraglide/messages';
	import PreferenceControl from '$lib/components/PreferenceControl.svelte';
	import { preferences } from '$lib/preferences/store.svelte';
	import {
		DEFAULT_PREFERENCES,
		type PreferenceKey,
		type Preferences
	} from '$lib/preferences/schema';

	type Tab = 'general' | 'accessibility';
	let activeTab = $state<Tab>('general');

	// The staged set the controls bind to. Initialised from the persisted set once
	// the store has hydrated, and re-synced to the persisted baseline after every
	// Apply / Discard / Reset so `dirty` returns to false. Shared across both tabs.
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

	const localeOptions = [
		{ value: 'en', label: m.prefs_locale_en() },
		{ value: 'de', label: m.prefs_locale_de() }
	];
	const textSizeOptions = [
		{ value: 'auto', label: m.prefs_option_auto() },
		{ value: 'small', label: m.prefs_option_small() },
		{ value: 'regular', label: m.prefs_option_regular() },
		{ value: 'large', label: m.prefs_option_large() }
	];
	const triStateOptions = [
		{ value: 'auto', label: m.prefs_option_auto_system() },
		{ value: 'on', label: m.prefs_option_on() },
		{ value: 'off', label: m.prefs_option_off() }
	];
	const toggleOptions = [
		{ value: 'off', label: m.prefs_option_off() },
		{ value: 'on', label: m.prefs_option_on() }
	];

	const tabs: ReadonlyArray<{ id: Tab; label: string }> = [
		{ id: 'general', label: m.prefs_tab_general() },
		{ id: 'accessibility', label: m.prefs_tab_accessibility() }
	];
</script>

<svelte:head><title>{m.prefs_page_title()} · E-R Bridge</title></svelte:head>

<main class="preferences">
	<h1>{m.prefs_heading()}</h1>
	<p class="intro">{m.prefs_intro()}</p>

	<div class="tabs" role="tablist" aria-label={m.prefs_heading()}>
		{#each tabs as tab (tab.id)}
			<button
				type="button"
				role="tab"
				id={`tab-${tab.id}`}
				aria-selected={activeTab === tab.id}
				aria-controls={`panel-${tab.id}`}
				class="tab"
				class:active={activeTab === tab.id}
				onclick={() => (activeTab = tab.id)}
			>
				{tab.label}
			</button>
		{/each}
	</div>

	{#if activeTab === 'general'}
		<div role="tabpanel" id="panel-general" aria-labelledby="tab-general">
			<PreferenceControl
				label={m.prefs_locale_label()}
				description={m.prefs_locale_description()}
				value={staged.locale}
				options={localeOptions}
				onSelect={(v) => select('locale', v)}
			/>
		</div>
	{:else}
		<div role="tabpanel" id="panel-accessibility" aria-labelledby="tab-accessibility">
			<PreferenceControl
				label={m.prefs_text_size_label()}
				description={m.prefs_text_size_description()}
				value={staged.text_size}
				options={textSizeOptions}
				onSelect={(v) => select('text_size', v)}
			/>

			<PreferenceControl
				label={m.prefs_reduce_motion_label()}
				description={m.prefs_reduce_motion_description()}
				value={staged.reduce_motion}
				options={triStateOptions}
				onSelect={(v) => select('reduce_motion', v)}
			/>

			<PreferenceControl
				label={m.prefs_high_contrast_label()}
				description={m.prefs_high_contrast_description()}
				value={staged.high_contrast}
				options={triStateOptions}
				onSelect={(v) => select('high_contrast', v)}
			/>

			<PreferenceControl
				label={m.prefs_large_targets_label()}
				description={m.prefs_large_targets_description()}
				value={staged.large_targets}
				options={toggleOptions}
				onSelect={(v) => select('large_targets', v)}
			/>

			<PreferenceControl
				label={m.prefs_dyslexia_font_label()}
				description={m.prefs_dyslexia_font_description()}
				value={staged.dyslexia_font}
				options={toggleOptions}
				onSelect={(v) => select('dyslexia_font', v)}
			/>
		</div>
	{/if}

	<div class="actions">
		{#if dirty}
			<button type="button" class="btn apply" onclick={apply}>{m.prefs_action_apply()}</button>
			<button type="button" class="btn discard" onclick={discard}>{m.prefs_action_discard()}</button>
		{/if}
		<button type="button" class="btn reset" onclick={reset}>{m.prefs_action_reset()}</button>
	</div>
</main>

<style>
	.preferences {
		/* Own scroll region inside the fixed-height app shell — see /about for the
		   rationale. flex:1 + min-height:0 fill remaining height and allow shrink;
		   the column stays centred via max-width + auto margins. The 960px width
		   matches /about and /characters so the content column is consistent. */
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		width: 100%;
		max-width: 960px;
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

	.tabs {
		display: flex;
		gap: 4px;
		margin-bottom: 8px;
		border-bottom: 1px solid var(--space-700);
	}

	.tab {
		padding: 8px 14px;
		font: inherit;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--slate-400);
		background: transparent;
		border: 0;
		border-bottom: 2px solid transparent;
		margin-bottom: -1px;
		cursor: pointer;
	}
	.tab:hover {
		color: var(--slate-100);
	}
	.tab.active {
		color: var(--sky);
		border-bottom-color: var(--sky);
	}
	.tab:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
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
