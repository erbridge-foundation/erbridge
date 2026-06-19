<script lang="ts">
	import { m } from '$lib/paraglide/messages';
</script>

<svelte:head>
	<title>{m.blocked_title()}</title>
</svelte:head>

<main class="body">
	<div class="panel" role="alert">
		<svg
			class="icon"
			width="32"
			height="32"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="1.5"
			aria-hidden="true"
		>
			<circle cx="12" cy="12" r="9"></circle>
			<line x1="5.6" y1="5.6" x2="18.4" y2="18.4"></line>
		</svg>
		<h1>{m.blocked_heading()}</h1>
		<p>{m.blocked_body()}</p>
		<a class="btn" href="/login">{m.blocked_back()}</a>
	</div>
</main>

<style>
	/* The chrome-less app shell (.app.chromeless in +layout.svelte) centers this
	   block in the viewport, mirroring how /login centers its card. The panel only
	   needs its own padding here. */
	.body {
		padding: 24px;
	}

	.panel {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		max-width: 420px;
		padding: 32px;
		text-align: center;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 8px;
	}
	.icon {
		color: var(--red);
	}
	h1 {
		margin: 0;
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--slate-100);
	}
	p {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.6;
		color: var(--slate-300);
	}
	.btn {
		margin-top: 8px;
		padding: 8px 16px;
		border: 1px solid var(--space-700);
		border-radius: 4px;
		color: var(--sky);
		font-size: 0.75rem;
		text-decoration: none;
	}
	.btn:hover {
		background: var(--space-700);
	}
</style>
