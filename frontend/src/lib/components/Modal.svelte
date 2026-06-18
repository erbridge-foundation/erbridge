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
		/** Backdrop style. `dim` (default) is the opaque dark scrim used by form
		 *  dialogs. `blur` keeps the content behind visible (blurred + lightly dimmed),
		 *  for dialogs whose edits should preview live against what is behind them. */
		backdrop?: 'dim' | 'blur';
		/** Dialog max-width. `medium` (default, 440px) suits most forms; `small` (360px)
		 *  for terse dialogs, `large` (640px) for wider content. */
		size?: 'small' | 'medium' | 'large';
	};

	let { open, title, children, onClose, backdrop = 'dim', size = 'medium' }: Props = $props();

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

	// Selector for the focusable set. Recomputed on every Tab (not cached) so
	// fields added or removed by conditional rendering always participate.
	const FOCUSABLE =
		'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

	function isVisible(el: HTMLElement): boolean {
		// Exclude elements hidden via the `hidden` attribute or a display:none /
		// visibility:hidden ancestor. We avoid offsetParent here because jsdom (and
		// position:fixed contexts) report it null even for visible elements.
		if (el.hidden) return false;
		if (typeof getComputedStyle !== 'function') return true;
		const style = getComputedStyle(el);
		return style.display !== 'none' && style.visibility !== 'hidden';
	}

	function focusableElements(): HTMLElement[] {
		if (!dialogEl) return [];
		return Array.from(dialogEl.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(isVisible);
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onClose();
			return;
		}

		if (event.key !== 'Tab') {
			return;
		}

		// Focus trap: cycle within the dialog's focusable elements. The set is
		// computed at keypress time so conditionally rendered fields are included.
		const focusables = focusableElements();
		if (focusables.length === 0) {
			// Nothing focusable inside; keep focus on the dialog itself.
			event.preventDefault();
			dialogEl?.focus();
			return;
		}

		const first = focusables[0];
		const last = focusables[focusables.length - 1];
		const active = document.activeElement;

		if (event.shiftKey) {
			if (active === first || active === dialogEl) {
				event.preventDefault();
				last.focus();
			}
		} else if (active === last) {
			event.preventDefault();
			first.focus();
		}
		// Otherwise let the browser move focus normally within the dialog.
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
		class:blur={backdrop === 'blur'}
		onpointerdown={onBackdropPointerDown}
		onkeydown={onKeydown}
		role="presentation"
		transition:fade|global={{ duration: reduceMotion ? 0 : 150 }}
	>
		<div
			bind:this={dialogEl}
			class="dialog"
			data-size={size}
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
	/* The `blur` variant keeps what is behind visible (blurred + lightly dimmed) so a
	   dialog whose edits preview live shows them happening behind it. */
	.backdrop.blur {
		background: rgba(0, 0, 0, 0.25);
		backdrop-filter: blur(4px);
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
	.dialog[data-size='small'] {
		max-width: 360px;
	}
	.dialog[data-size='large'] {
		max-width: 640px;
	}

	.title {
		margin: 0;
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--slate-100);
	}
</style>
