<!--
	Modal.svelte — generic dialog shell for forms (create map, etc.).

	ConfirmDialog is purpose-built for a two-button confirm/cancel with a
	two-focusable trap; it does not fit a dialog that contains form fields. This
	component is the form-bearing sibling: a backdrop + a labelled dialog that
	renders an arbitrary `children` snippet, closes on Escape or backdrop click,
	and restores focus to the opener on close.

	Motion honours the same reduce-motion tri-state as ConfirmDialog.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade, scale } from 'svelte/transition';

	type Props = {
		open: boolean;
		title: Snippet;
		children: Snippet;
		onClose: () => void;
	};

	let { open, title, children, onClose }: Props = $props();

	const instanceId = $props.id();
	const titleId = `${instanceId}-title`;

	let dialogEl = $state<HTMLDivElement | null>(null);
	let previouslyFocusedEl: HTMLElement | null = null;

	function prefersReducedMotion(): boolean {
		if (typeof document !== 'undefined') {
			const override = document.documentElement.getAttribute('data-reduce-motion');
			if (override === 'on') return true;
			if (override === 'off') return false;
		}
		if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
			return false;
		}
		return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
	}

	let reduceMotion = $state(false);

	$effect(() => {
		if (!open) return;
		previouslyFocusedEl = (document.activeElement as HTMLElement | null) ?? null;
		reduceMotion = prefersReducedMotion();
		return () => {
			previouslyFocusedEl?.focus();
			previouslyFocusedEl = null;
		};
	});

	// Move focus to the first focusable field inside the dialog on open.
	$effect(() => {
		if (open && dialogEl) {
			const first = dialogEl.querySelector<HTMLElement>(
				'input, textarea, select, button, [tabindex]:not([tabindex="-1"])'
			);
			first?.focus();
		}
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
		}
	}

	function onBackdropPointerDown(event: PointerEvent) {
		if (event.target === event.currentTarget) {
			onClose();
		}
	}

	function onDialogPointerDown(event: PointerEvent) {
		event.stopPropagation();
	}
</script>

{#if open}
	<div
		class="backdrop"
		onpointerdown={onBackdropPointerDown}
		onkeydown={onKeydown}
		role="presentation"
		transition:fade|global={{ duration: reduceMotion ? 0 : 150 }}
	>
		<div
			bind:this={dialogEl}
			class="dialog"
			role="dialog"
			aria-modal="true"
			aria-labelledby={titleId}
			tabindex="-1"
			onpointerdown={onDialogPointerDown}
			transition:scale|global={{ duration: reduceMotion ? 0 : 150, start: 0.96, opacity: 0 }}
		>
			<h2 id={titleId} class="title">{@render title()}</h2>
			{@render children()}
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
		background: rgba(0, 0, 0, 0.6);
	}

	.dialog {
		width: 100%;
		max-width: 440px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 24px;
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		box-shadow: 0 24px 48px rgba(0, 0, 0, 0.5);
	}

	.title {
		margin: 0;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--slate-100);
	}
</style>
