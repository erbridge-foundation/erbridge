<!--
	/preferences — accessibility settings.

	Reachable anonymously (settings work before/without login; see +layout.server
	public-route list). Layout-altering changes (text_size, high_contrast,
	large_targets, dyslexia_font) are applied as a live preview and confirmed via
	PreferenceRevertBar, which auto-reverts if the user does nothing — so a setting
	that breaks the page recovers itself. reduce_motion commits immediately (it
	cannot lock a user out).
-->
<script lang="ts">
	import PreferenceControl from '$lib/components/PreferenceControl.svelte';
	import PreferenceRevertBar from '$lib/components/PreferenceRevertBar.svelte';
	import { preferences } from '$lib/preferences/store.svelte';
	import {
		LAYOUT_ALTERING_KEYS,
		type PreferenceKey,
		type PreferencesPatch
	} from '$lib/preferences/schema';

	// A pending layout-altering change awaiting confirmation (null = none).
	let pending = $state<PreferencesPatch | null>(null);

	const current = $derived(preferences.current);

	function isLayoutAltering(key: PreferenceKey): boolean {
		return LAYOUT_ALTERING_KEYS.includes(key);
	}

	function select(key: PreferenceKey, value: string) {
		const patch = { [key]: value } as PreferencesPatch;

		if (isLayoutAltering(key)) {
			// Apply as a preview and ask for confirmation. If another change is
			// already pending, this replaces it (the new preview is what's shown).
			preferences.preview(patch);
			pending = patch;
		} else {
			// reduce_motion: commit immediately, no countdown.
			void preferences.commit(patch);
		}
	}

	function keep() {
		const patch = pending;
		pending = null;
		if (patch) void preferences.commit(patch);
	}

	function revert() {
		pending = null;
		preferences.revertToPersisted();
	}

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
		These settings apply to this browser immediately. When you're signed in they're
		saved to your account and follow you across devices.
	</p>

	<PreferenceControl
		label="Text size"
		description="Scales all text in the interface. Auto follows your browser's default size."
		value={current.text_size}
		options={textSizeOptions}
		onSelect={(v) => select('text_size', v)}
	/>

	<PreferenceControl
		label="Reduce motion"
		description="Disables animations and transitions. Auto follows your system setting."
		value={current.reduce_motion}
		options={triStateOptions}
		onSelect={(v) => select('reduce_motion', v)}
	/>

	<PreferenceControl
		label="High contrast"
		description="Increases contrast between text and background. Auto follows your system setting."
		value={current.high_contrast}
		options={triStateOptions}
		onSelect={(v) => select('high_contrast', v)}
	/>

	<PreferenceControl
		label="Larger interactive targets"
		description="Increases the minimum size of buttons and links to make them easier to hit."
		value={current.large_targets}
		options={toggleOptions}
		onSelect={(v) => select('large_targets', v)}
	/>

	<PreferenceControl
		label="Dyslexia-friendly typeface"
		description="Replaces the interface font with Atkinson Hyperlegible."
		value={current.dyslexia_font}
		options={toggleOptions}
		onSelect={(v) => select('dyslexia_font', v)}
	/>
</main>

{#if pending}
	<PreferenceRevertBar onKeep={keep} onRevert={revert} />
{/if}

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
</style>
