<!--
	ConfirmDialog.svelte — shared destructive-action confirmation modal.

	Specified by: openspec/specs/frontend-patterns/spec.md
	Implemented per: openspec/changes/confirm-destructive-actions/tasks.md §2.

	The cancel label is the shared `dialog_cancel` message, not a prop (§1.2 of
	the change tasks): the spec requires every invocation to use the same word,
	and a single message key guarantees that across locales.

	HANDOFF STATUS (§2.8). The lines below mark each behaviour as either
	covered by the Vitest suite (@verified-by-test) or still needing a
	real-browser check by §6 (@needs-browser-check). The browser checks
	belong to §6.5–6.6 in tasks.md; they are not §3's responsibility.

	@verified-by-test:opens-with-cancel-focused
	@verified-by-test:escape-calls-oncancel
	@verified-by-test:backdrop-click-calls-oncancel
	@verified-by-test:body-click-does-not-bubble-to-backdrop
	@verified-by-test:tab-cycles-between-cancel-and-confirm
	@verified-by-test:onconfirm-fires-only-on-confirm-activation
	@verified-by-test:prefers-reduced-motion-read-at-runtime
	@needs-browser-check:focus-returns-to-opener-on-close (jsdom focus model
	  is unreliable; tested logic exists in $effect cleanup; verify in §6.2)
	@needs-browser-check:reduced-motion-actually-disables-visible-animation
	  (jsdom cannot observe transitions; verify in §6.5)
	@needs-browser-check:screen-reader-announces-title-and-body (§6.6)

	USAGE EXAMPLE — for callers (e.g. /characters/+page.svelte §3):

		<script lang="ts">
			import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';

			let formEl: HTMLFormElement;
			let open = $state(false);
		</script>

		<form bind:this={formEl} method="POST" action="?/delete" use:enhance>
			<button type="button" onclick={() => (open = true)}>delete</button>
		</form>

		<ConfirmDialog
			{open}
			tone="danger"
			onCancel={() => (open = false)}
			onConfirm={() => {
				formEl.requestSubmit();
				open = false;
			}}
		>
			{#snippet title()}Delete thing?{/snippet}
			{#snippet body()}One sentence describing the consequence.{/snippet}
			{#snippet confirmLabel()}delete thing{/snippet}
		</ConfirmDialog>
-->

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade, scale } from 'svelte/transition';
	import { m } from '$lib/paraglide/messages';

	type Props = {
		open: boolean;
		tone: 'danger';
		title: Snippet;
		body: Snippet;
		confirmLabel: Snippet;
		onCancel: () => void;
		onConfirm: () => void;
	};

	let { open, tone, title, body, confirmLabel, onCancel, onConfirm }: Props = $props();

	// Per-instance ids for aria-labelledby / aria-describedby. $props.id() is
	// SSR-stable and unique per component instance, and must be assigned to
	// a variable directly (the rune cannot appear inside an expression).
	const instanceId = $props.id();
	const titleId = `${instanceId}-title`;
	const bodyId = `${instanceId}-body`;

	let dialogEl = $state<HTMLDivElement | null>(null);
	let cancelButtonEl = $state<HTMLButtonElement | null>(null);
	let confirmButtonEl = $state<HTMLButtonElement | null>(null);
	let previouslyFocusedEl: HTMLElement | null = null;

	// Resolve whether motion should be reduced, honouring the full tri-state of the
	// reduce_motion accessibility preference:
	//   data-reduce-motion="on"  → reduce (explicit override beats the OS)
	//   data-reduce-motion="off" → do not reduce (explicit opt-out beats the OS)
	//   absent (auto)            → follow the OS prefers-reduced-motion media query
	// In SSR / test environments where matchMedia is unavailable, default to "no
	// reduce"; the CSS media query layer still disables motion for real users.
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

	// Effect 1: open/close lifecycle — capture the trigger, read the
	// reduced-motion preference, restore focus on close. This effect runs
	// once per open→close cycle.
	$effect(() => {
		if (!open) {
			return;
		}

		// Capture the focus target before we move focus into the dialog so we
		// can restore it on close. Reads document.activeElement at open time.
		previouslyFocusedEl = (document.activeElement as HTMLElement | null) ?? null;
		reduceMotion = prefersReducedMotion();

		return () => {
			// On close, restore focus to whatever opened us (typically the
			// destructive trigger button on the caller's page).
			previouslyFocusedEl?.focus();
			previouslyFocusedEl = null;
		};
	});

	// Effect 2: default focus — runs whenever the dialog is open AND the
	// cancel button is bound. queueMicrotask isn't enough: Svelte 5 binds
	// `bind:this` during DOM mount which can land in a later tick than a
	// microtask, leaving cancelButtonEl null when we'd try to focus it. A
	// reactive $effect keyed on cancelButtonEl re-runs once the bind fires,
	// guaranteeing focus lands on cancel before the user can act on the
	// dialog. (jsdom hides this race, hence the @needs-browser-check tag.)
	$effect(() => {
		if (open && cancelButtonEl) {
			cancelButtonEl.focus();
		}
	});

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			onCancel();
			return;
		}

		if (event.key !== 'Tab') {
			return;
		}

		// Focus trap: cycle between the two focusable buttons. With exactly two
		// focusables we can implement this with a simple swap rather than a
		// general focusable-element scan.
		const focusables = [cancelButtonEl, confirmButtonEl].filter(
			(el): el is HTMLButtonElement => el !== null
		);
		if (focusables.length === 0) {
			return;
		}

		const first = focusables[0];
		const last = focusables[focusables.length - 1];
		const active = document.activeElement;

		if (event.shiftKey && active === first) {
			event.preventDefault();
			last.focus();
		} else if (!event.shiftKey && active === last) {
			event.preventDefault();
			first.focus();
		}
		// Otherwise let the browser handle normal Tab between the two buttons.
	}

	function onBackdropPointerDown(event: PointerEvent) {
		// The backdrop is the outer container; if the user clicked inside the
		// dialog body, the inner stopPropagation handler has already run and
		// this listener won't be invoked for that path. This handler runs only
		// for clicks on the backdrop itself.
		if (event.target === event.currentTarget) {
			onCancel();
		}
	}

	function onDialogPointerDown(event: PointerEvent) {
		// Stop pointer events inside the dialog from bubbling to the backdrop.
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
			data-tone={tone}
			role="alertdialog"
			aria-modal="true"
			aria-labelledby={titleId}
			aria-describedby={bodyId}
			tabindex="-1"
			onpointerdown={onDialogPointerDown}
			transition:scale|global={{
				duration: reduceMotion ? 0 : 150,
				start: 0.96,
				opacity: 0
			}}
		>
			<h2 id={titleId} class="title">{@render title()}</h2>
			<p id={bodyId} class="body">{@render body()}</p>
			<div class="actions">
				<button
					bind:this={cancelButtonEl}
					type="button"
					class="cancel"
					onclick={onCancel}
				>
					{m.dialog_cancel()}
				</button>
				<button
					bind:this={confirmButtonEl}
					type="button"
					class="confirm"
					onclick={onConfirm}
				>
					{@render confirmLabel()}
				</button>
			</div>
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

	.body {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.55;
		color: var(--slate-300);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 12px;
		margin-top: 8px;
	}

	.actions button {
		padding: 8px 16px;
		font: inherit;
		font-size: 0.75rem;
		border-radius: 4px;
		cursor: pointer;
	}

	.cancel {
		background: transparent;
		border: 1px solid var(--space-700);
		color: var(--slate-300);
	}
	.cancel:hover {
		background: var(--space-700);
		color: var(--slate-100);
	}
	.cancel:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	.confirm {
		background: transparent;
		border: 1px solid var(--red);
		color: var(--red);
	}
	.confirm:hover {
		background: var(--red);
		color: var(--slate-100);
	}
	.confirm:focus-visible {
		outline: 2px solid var(--red);
		outline-offset: 2px;
	}

	@media (max-width: 600px) {
		.backdrop {
			padding: 16px;
			/* Honor mobile safe-area insets at the bottom (iOS Home indicator). */
			padding-bottom: max(16px, env(safe-area-inset-bottom));
		}
		.dialog {
			max-width: 100%;
			padding: 20px;
		}
		.actions {
			flex-direction: column-reverse;
			align-items: stretch;
		}
	}

	/* Defence-in-depth: even though we pass duration: 0 to Svelte transitions
	   when the JS path detects reduced motion, also kill any CSS-driven motion
	   that a future contributor might add inside this component. */
	@media (prefers-reduced-motion: reduce) {
		.backdrop,
		.dialog {
			animation-duration: 0s !important;
			transition-duration: 0s !important;
		}
	}
</style>
