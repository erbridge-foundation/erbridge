<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	let main = $derived(data.me?.characters.find((c) => c.is_main) ?? null);
</script>

<main class="body">
	<div class="content">
		{#if main}
			<h1 class="welcome">{m.home_welcome_named({ name: main.name })}</h1>

			<section class="main-character" aria-label={m.home_main_character()}>
				<div class="name-row">
					<span>{main.name}</span>
					<span class="sep">·</span>
					<span class="role">{m.home_role_main()}</span>
				</div>
				<div class="corp">{main.corporation_name}</div>
				{#if main.alliance_name}
					<div class="alliance">{main.alliance_name}</div>
				{/if}
			</section>
		{:else}
			<h1 class="welcome">{m.home_welcome_anonymous()}</h1>
		{/if}
	</div>
</main>

<style>
	.body {
		flex: 1;
		overflow: auto;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.content {
		width: 100%;
		max-width: 480px;
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
	}

	.welcome {
		margin: 0 0 32px;
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--sky);
	}

	.main-character {
		display: flex;
		flex-direction: column;
		gap: 4px;
		align-items: center;
	}

	.name-row {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 0.875rem;
		color: var(--slate-100);
		font-weight: 600;
	}

	.sep {
		color: var(--slate-500);
		font-weight: 400;
	}

	.role {
		color: var(--slate-300);
		font-weight: 400;
	}

	.corp,
	.alliance {
		font-size: 0.875rem;
		color: var(--slate-400);
	}
</style>
