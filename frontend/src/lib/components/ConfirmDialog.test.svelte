<!--
	Test harness for ConfirmDialog. Snippets cannot be constructed from plain
	JavaScript test code (they're a compile-time construct), so we render
	ConfirmDialog through this thin wrapper and pass props in via $props().

	Used only by ConfirmDialog.test.ts.
-->

<script lang="ts">
	import ConfirmDialog from './ConfirmDialog.svelte';

	type Props = {
		open: boolean;
		onCancel: () => void;
		onConfirm: () => void;
		titleText?: string;
		bodyText?: string;
		confirmLabelText?: string;
	};

	let {
		open,
		onCancel,
		onConfirm,
		titleText = 'Delete thing?',
		bodyText = 'This will permanently remove the thing.',
		confirmLabelText = 'delete thing'
	}: Props = $props();
</script>

<!-- Provide an external trigger so we can assert focus restoration on close. -->
<button type="button" data-testid="opener">opener</button>

<ConfirmDialog {open} tone="danger" {onCancel} {onConfirm}>
	{#snippet title()}{titleText}{/snippet}
	{#snippet body()}{bodyText}{/snippet}
	{#snippet confirmLabel()}{confirmLabelText}{/snippet}
</ConfirmDialog>
