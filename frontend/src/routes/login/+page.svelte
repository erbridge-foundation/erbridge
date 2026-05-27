<script lang="ts">
	import { m } from '$lib/paraglide/messages';
</script>

<svelte:head>
	<title>{m.login_title()}</title>
</svelte:head>

<div class="card" role="main">
	<svg
		class="logo"
		width="28"
		height="28"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="1.5"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="3"></circle>
		<path d="M12 2v4M12 18v4M2 12h4M18 12h4"></path>
		<path d="M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M18.4 5.6l-2.8 2.8M8.4 15.6l-2.8 2.8"></path>
	</svg>

	<h1 class="brand-name">E-R BRIDGE</h1>
	<p class="subtitle">{m.login_subtitle()}</p>

	<hr class="divider" />

	<a href="/auth/login" class="sso-link" aria-label={m.login_sso_aria()}>
		<img
			src="https://web.ccpgamescdn.com/eveonlineassets/developers/eve-sso-login-white-large.png"
			alt="LOG IN with EVE Online"
			width="270"
		/>
	</a>

	<p class="disclaimer">
		{m.login_disclaimer_line1()}<br />
		{m.login_disclaimer_line2()}
	</p>
</div>

<style>
	.card {
		width: 360px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 8px;
		padding: 32px 24px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
	}

	.logo {
		color: var(--sky);
	}

	.brand-name {
		margin: 8px 0 0;
		font-size: 0.875rem;
		font-weight: 600;
		letter-spacing: 0.2em;
		color: var(--slate-100);
	}

	.subtitle {
		margin: 0;
		font-size: 0.75rem;
		font-weight: 400;
		color: var(--slate-400);
	}

	.divider {
		width: 100%;
		margin: 16px 0 8px;
		border: 0;
		border-top: 1px solid var(--space-700);
	}

	.sso-link {
		display: inline-block;
		line-height: 0;
	}
	.sso-link img {
		display: block;
		height: auto;
		max-width: 100%;
	}

	.disclaimer {
		margin: 8px 0 0;
		font-size: 0.75rem;
		color: var(--slate-500);
		text-align: center;
		line-height: 1.6;
	}
</style>
