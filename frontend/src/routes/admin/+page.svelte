<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head>
	<title>{m.admin_overview_title()}</title>
</svelte:head>

<h1 class="page-heading">{m.admin_overview_heading()}</h1>

<div class="cards">
	<a class="stat-card" href="/admin/admins">
		<span class="stat-value">{data.adminCount}</span>
		<span class="stat-label">{m.admin_overview_admins_label()}</span>
		<span class="stat-cta">{m.admin_overview_admins_cta()} →</span>
	</a>
	<a class="stat-card" href="/admin/blocks">
		<span class="stat-value">{data.blockCount}</span>
		<span class="stat-label">{m.admin_overview_blocks_label()}</span>
		<span class="stat-cta">{m.admin_overview_blocks_cta()} →</span>
	</a>
	<a class="stat-card" href="/admin/audit">
		<span class="stat-label">{m.admin_nav_audit()}</span>
		<span class="stat-cta">{m.admin_overview_audit_cta()} →</span>
	</a>
</div>

<style>
	.page-heading {
		margin: 0 0 16px;
		font-size: 0.75rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-500);
	}

	.cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
		gap: 16px;
	}
	@media (max-width: 600px) {
		.cards {
			grid-template-columns: 1fr;
		}
	}

	.stat-card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 20px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		text-decoration: none;
	}
	.stat-card:hover {
		border-color: var(--sky);
	}
	.stat-value {
		font-size: 2rem;
		font-weight: 600;
		color: var(--slate-100);
		line-height: 1;
	}
	.stat-label {
		font-size: 0.75rem;
		color: var(--slate-400);
	}
	.stat-cta {
		margin-top: 8px;
		font-size: 0.75rem;
		color: var(--sky);
	}
</style>
