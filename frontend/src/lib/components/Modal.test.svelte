<!--
	Test harness for Modal. Snippets cannot be constructed from plain JavaScript
	test code (they're a compile-time construct), so we render Modal through this
	thin wrapper and pass props in via $props().

	The children snippet renders a small form whose third field is conditionally
	rendered, so the focus-trap tests can assert that a field added while the
	dialog is open joins the cycle.

	Used only by Modal.test.ts.
-->

<script lang="ts">
	import Modal from './Modal.svelte';

	type Props = {
		open: boolean;
		onClose: () => void;
		titleText?: string;
		showExtra?: boolean;
	};

	let { open, onClose, titleText = 'Create map', showExtra = false }: Props = $props();
</script>

<!-- Provide an external trigger so we can assert focus restoration on close. -->
<button type="button" data-testid="opener">opener</button>

<Modal {open} {onClose}>
	{#snippet title()}{titleText}{/snippet}
	{#snippet children()}
		<input type="text" name="name" data-testid="first" />
		<input type="text" name="slug" data-testid="second" />
		{#if showExtra}
			<input type="text" name="extra" data-testid="extra" />
		{/if}
		<button type="submit" data-testid="last">create</button>
	{/snippet}
</Modal>
