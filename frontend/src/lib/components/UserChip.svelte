<script lang="ts">
	import UserMenu from './UserMenu.svelte';

	let { portraitUrl, name }: { portraitUrl: string; name: string } = $props();

	let open = $state(false);
	let chipAnchor: HTMLDivElement;

	function toggle() {
		open = !open;
	}

	function close() {
		open = false;
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}

	function onDocumentClick(e: MouseEvent) {
		if (chipAnchor && !chipAnchor.contains(e.target as Node)) {
			close();
		}
	}

	$effect(() => {
		if (open) {
			document.addEventListener('click', onDocumentClick);
			document.addEventListener('keydown', onKeydown);
		} else {
			document.removeEventListener('click', onDocumentClick);
			document.removeEventListener('keydown', onKeydown);
		}
		return () => {
			document.removeEventListener('click', onDocumentClick);
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<div class="chip-anchor" bind:this={chipAnchor}>
	<button
		class="user-chip"
		class:open
		type="button"
		aria-haspopup="menu"
		aria-expanded={open}
		aria-controls="user-menu"
		onclick={toggle}
	>
		<img src={portraitUrl} alt="" width="24" height="24" />
		<span class="name">{name}</span>
		<svg
			class="caret"
			class:flipped={open}
			width="12"
			height="12"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			aria-hidden="true"
		>
			<polyline points="6 9 12 15 18 9"></polyline>
		</svg>
	</button>

	{#if open}
		<UserMenu onclose={close} />
	{/if}
</div>

<style>
	.chip-anchor {
		position: relative;
	}

	.user-chip {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 4px 8px;
		border-radius: 4px;
		color: var(--slate-200);
		cursor: pointer;
		background: transparent;
		border: 0;
		font: inherit;
	}
	.user-chip:hover,
	.user-chip.open {
		background: var(--space-700);
	}

	.user-chip img {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		display: block;
	}

	.name {
		font-size: 0.75rem;
	}

	.caret {
		color: var(--slate-500);
		transition: transform 0.15s ease;
	}
	.caret.flipped {
		transform: rotate(180deg);
		color: var(--slate-300);
	}
</style>
