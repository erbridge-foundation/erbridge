<!--
	PreferenceControl.svelte — a labelled segmented control for one preference.

	A row of radio-style buttons (one per allowed value). Emits the chosen value
	via onSelect; the parent decides whether to commit immediately or preview +
	confirm (for layout-altering preferences).
-->
<script lang="ts">
	type Props = {
		label: string;
		description: string;
		value: string;
		options: ReadonlyArray<{ value: string; label: string }>;
		onSelect: (value: string) => void;
	};

	let { label, description, value, options, onSelect }: Props = $props();

	const groupId = $props.id();
</script>

<fieldset class="control">
	<legend class="label" id={`${groupId}-label`}>{label}</legend>
	<p class="description">{description}</p>
	<div class="options" role="radiogroup" aria-labelledby={`${groupId}-label`}>
		{#each options as opt (opt.value)}
			<button
				type="button"
				role="radio"
				aria-checked={value === opt.value}
				class="option"
				class:selected={value === opt.value}
				onclick={() => onSelect(opt.value)}
			>
				{opt.label}
			</button>
		{/each}
	</div>
</fieldset>

<style>
	.control {
		margin: 0;
		padding: 16px 0;
		border: 0;
		border-bottom: 1px solid var(--space-700);
	}

	.label {
		padding: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-100);
	}

	.description {
		margin: 4px 0 12px;
		font-size: 0.75rem;
		color: var(--slate-400);
	}

	.options {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.option {
		padding: 8px 16px;
		font: inherit;
		font-size: 0.75rem;
		color: var(--slate-300);
		background: var(--space-800);
		border: 1px solid var(--space-600);
		border-radius: 4px;
		cursor: pointer;
	}
	.option:hover {
		background: var(--space-700);
		color: var(--slate-100);
	}
	.option.selected {
		color: var(--space-950);
		background: var(--sky);
		border-color: var(--sky);
	}
	.option:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}
</style>
