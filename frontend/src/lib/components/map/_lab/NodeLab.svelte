<script lang="ts">
	// NODE LAB — a DISPOSABLE wireframe gallery for comparing SystemNode designs. The
	// current node reads "fat" (stacked full-width badge rows → tall); this lays five
	// variants side-by-side over the SAME sample systems so we can eyeball which slim
	// layout to adopt. Not a real canvas — a static comparison grid. Deleted once a
	// direction is chosen (the winner folds back into SystemNode.svelte).
	import { m } from '$lib/paraglide/messages';
	import type { System } from '$lib/map/types';
	import NodeBaseline from './NodeBaseline.svelte';
	import NodeWanderer from './NodeWanderer.svelte';
	import NodeSingleLine from './NodeSingleLine.svelte';
	import NodeStripeHybrid from './NodeStripeHybrid.svelte';
	import NodeMinimal from './NodeMinimal.svelte';

	// Sample systems chosen to exercise what drives node size: a plain WH, a custom-named
	// WH, a k-space system, a multi-flag system (the compose case), a 2-static system, a
	// long custom name, and a root. `flags` is shared intel; `custom_name` is the user's
	// blue label. (Self-contained — the lab doesn't touch the shared fixture.)
	const base = (over: Partial<System>): System => ({
		id: 'J120455',
		name: 'J120455',
		eve_system_id: null,
		class: 'C2',
		statics: [],
		scans: [],
		structures: [],
		...over
	});

	type Sample = { label: string; system: System; isRoot?: boolean; selected?: boolean };
	const samples: Sample[] = [
		{ label: 'Plain WH', system: base({}) },
		{ label: 'Custom name', system: base({ custom_name: 'Deep X' }) },
		{
			label: 'k-space + custom',
			system: base({ id: 'Stou', name: 'Stou', class: 'LS', custom_name: 'Verge Vendor' })
		},
		{
			label: 'Multi-flag (compose)',
			system: base({
				id: 'J100004',
				name: 'J100004',
				class: 'C4',
				custom_name: 'Hostile camp',
				flags: ['looking-for', 'warning']
			})
		},
		{
			label: 'Two statics',
			system: base({
				id: 'J100005',
				name: 'J100005',
				class: 'C5',
				statics: [
					{ wh_type: 'H900', dest: 'C5' },
					{ wh_type: 'S199', dest: 'NS' }
				]
			})
		},
		{
			label: 'Root + flag',
			system: base({
				id: 'J172840',
				name: 'J172840',
				class: 'C5',
				custom_name: 'HOME',
				statics: [{ wh_type: 'H296', dest: 'C5' }],
				flags: ['friendly']
			}),
			isRoot: true
		},
		{
			label: 'Long custom name',
			system: base({
				id: 'J232934',
				name: 'J232934',
				class: 'C5',
				custom_name: 'High Bear farm — staging'
			})
		},
		{ label: 'Selected', system: base({ custom_name: 'Deep X' }), selected: true }
	];

	const variants = [
		{ key: 'baseline', label: m.map_proto_lab_v_baseline(), comp: NodeBaseline },
		{ key: 'wanderer', label: m.map_proto_lab_v_wanderer(), comp: NodeWanderer },
		{ key: 'single', label: m.map_proto_lab_v_single(), comp: NodeSingleLine },
		{ key: 'stripe', label: m.map_proto_lab_v_stripe(), comp: NodeStripeHybrid },
		{ key: 'minimal', label: m.map_proto_lab_v_minimal(), comp: NodeMinimal }
	] as const;
</script>

<section class="lab" aria-label={m.map_proto_lab_title()}>
	<header class="lab-head">
		<h2>{m.map_proto_lab_title()}</h2>
		<p class="note">{m.map_proto_lab_note()}</p>
	</header>

	<div class="grid" style:--cols={variants.length}>
		<!-- Column headers: the variant names. The first cell labels the sample column. -->
		<div class="cell head corner">{m.map_proto_lab_sample()}</div>
		{#each variants as v (v.key)}
			<div class="cell head">{v.label}</div>
		{/each}

		{#each samples as s (s.label + (s.selected ? '-sel' : ''))}
			<div class="cell row-label">{s.label}</div>
			{#each variants as v (v.key)}
				{@const Comp = v.comp}
				<div class="cell">
					<Comp system={s.system} isRoot={s.isRoot ?? false} selected={s.selected ?? false} />
				</div>
			{/each}
		{/each}
	</div>
</section>

<style>
	.lab {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: 1rem 1.25rem;
		background: var(--space-900);
	}
	.lab-head h2 {
		margin: 0;
		font-size: 1rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	.note {
		margin: 0.2rem 0 1rem;
		font-size: 0.8rem;
		color: var(--slate-400);
	}

	.grid {
		display: grid;
		grid-template-columns: minmax(120px, max-content) repeat(var(--cols), minmax(180px, 1fr));
		gap: 1px;
		background: var(--space-700);
		border: 1px solid var(--space-700);
		border-radius: 8px;
		overflow: hidden;
	}
	.cell {
		display: flex;
		align-items: center;
		padding: 0.75rem;
		/* Data cells match the container shade; the 1px --space-700 grid gap separates
		   them. The header + row-label cells use the lighter --space-800 to stand out. */
		background: var(--space-900);
	}
	.cell.head {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--slate-300);
		background: var(--space-800);
		position: sticky;
		top: 0;
		z-index: 1;
	}
	.cell.corner {
		color: var(--slate-500);
	}
	.cell.row-label {
		font-size: 0.78rem;
		color: var(--slate-400);
		background: var(--space-800);
	}
</style>
