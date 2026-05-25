<!--
	PreferenceRevertBar.svelte — the auto-reverting confirmation for layout-altering
	preference changes (text_size, high_contrast, large_targets, dyslexia_font).

	The change is applied as a live PREVIEW before this bar shows; the bar counts
	down PREFERENCE_REVERT_SECONDS and, if the user does nothing, calls onRevert.
	"Keep" calls onKeep (which persists), "Revert now" calls onRevert immediately.

	The bar is styled with FIXED px sizing and a guaranteed-contrast palette so the
	previewed change (which may enlarge text or alter contrast) cannot render the
	escape hatch itself unusable.
-->
<script lang="ts">
	import { PREFERENCE_REVERT_SECONDS } from '$lib/preferences/schema';

	type Props = {
		onKeep: () => void;
		onRevert: () => void;
	};

	let { onKeep, onRevert }: Props = $props();

	let remaining = $state(PREFERENCE_REVERT_SECONDS);
	let intervalId: ReturnType<typeof setInterval> | undefined;

	$effect(() => {
		intervalId = setInterval(() => {
			remaining -= 1;
			if (remaining <= 0) {
				clearInterval(intervalId);
				onRevert();
			}
		}, 1000);

		return () => clearInterval(intervalId);
	});
</script>

<div class="revert-bar" role="alertdialog" aria-live="assertive" aria-label="Confirm preference change">
	<span class="message">
		Preview applied. Keeping these settings in <strong>{remaining}</strong>s…
	</span>
	<span class="actions">
		<button type="button" class="keep" onclick={onKeep}>Keep</button>
		<button type="button" class="revert" onclick={onRevert}>Revert now</button>
	</span>
</div>

<style>
	/* All sizing is fixed px and the palette is hard-coded (not via tokens that
	   high_contrast overrides) so the bar stays usable under any previewed change. */
	.revert-bar {
		position: fixed;
		left: 50%;
		bottom: 24px;
		transform: translateX(-50%);
		z-index: 2000;
		display: flex;
		align-items: center;
		gap: 16px;
		max-width: 520px;
		padding: 12px 16px;
		background: #0d1526;
		border: 2px solid #38bdf8;
		border-radius: 8px;
		box-shadow: 0 12px 32px rgba(0, 0, 0, 0.6);
		font-family: ui-sans-serif, system-ui, sans-serif;
		font-size: 14px;
		line-height: 1.4;
		color: #ffffff;
	}

	.message strong {
		color: #38bdf8;
	}

	.actions {
		display: flex;
		gap: 8px;
		flex-shrink: 0;
	}

	.actions button {
		padding: 8px 14px;
		font-family: inherit;
		font-size: 13px;
		border-radius: 4px;
		border: 1px solid #ffffff;
		cursor: pointer;
		min-height: 36px;
	}

	.keep {
		background: #38bdf8;
		border-color: #38bdf8;
		color: #05080f;
		font-weight: 600;
	}
	.revert {
		background: transparent;
		color: #ffffff;
	}
	.actions button:focus-visible {
		outline: 2px solid #ffffff;
		outline-offset: 2px;
	}
</style>
