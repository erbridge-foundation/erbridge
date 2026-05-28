<script lang="ts">
	import { updated } from '$app/state';
	import { m } from '$lib/paraglide/messages';

	// `onReload` is injectable so the Vitest component test can assert that
	// activating the control invokes a reload without actually reloading the
	// jsdom window. Real callers omit it and get the default `location.reload()`.
	type Props = {
		onReload?: () => void;
	};

	let { onReload = () => location.reload() }: Props = $props();
</script>

{#if updated.current}
	<div class="update-banner" role="status" aria-live="polite">
		<span class="message">{m.update_banner_message()}</span>
		<button type="button" class="reload" onclick={onReload}>
			{m.update_banner_reload()}
		</button>
	</div>
{/if}

<style>
	.update-banner {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 8px 16px;
		background: rgba(56, 189, 248, 0.08);
		border-bottom: 1px solid var(--sky);
		color: var(--sky);
		font-size: 0.75rem;
	}

	.message {
		flex: 1;
	}

	.reload {
		flex-shrink: 0;
		padding: 4px 12px;
		font: inherit;
		font-size: 0.75rem;
		background: transparent;
		border: 1px solid var(--sky);
		border-radius: 4px;
		color: var(--sky);
		cursor: pointer;
	}
	.reload:hover {
		background: var(--sky);
		color: var(--space-950);
	}
	.reload:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
</style>
