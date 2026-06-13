<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { preferences } from '$lib/preferences/store.svelte';
	import {
		DEFAULT_PREFERENCES,
		MAX_PREFERENCES,
		type Locale
	} from '$lib/preferences/schema';

	// The preset is "on" iff every one of its five keys currently matches MAX_PREFERENCES.
	const presetKeys = Object.keys(MAX_PREFERENCES) as (keyof typeof MAX_PREFERENCES)[];
	const presetActive = $derived(
		presetKeys.every((k) => preferences.current[k] === MAX_PREFERENCES[k])
	);

	function toggleMaxAccessibility() {
		if (presetActive) {
			// Revert just the five preset keys to their defaults (leave locale alone).
			const revert = Object.fromEntries(
				presetKeys.map((k) => [k, DEFAULT_PREFERENCES[k]])
			);
			preferences.commit(revert);
		} else {
			preferences.commit({ ...MAX_PREFERENCES });
		}
	}

	const localeOptions: ReadonlyArray<{ value: Locale; label: string }> = [
		{ value: 'en', label: m.prefs_locale_en() },
		{ value: 'de', label: m.prefs_locale_de() },
		{ value: 'fr', label: m.prefs_locale_fr() }
	];

	function onLocaleChange(event: Event) {
		const locale = (event.currentTarget as HTMLSelectElement).value as Locale;
		if (locale === preferences.current.locale) return;
		preferences.commit({ locale });
	}
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

	<hr class="divider" />

	<div class="controls">
		<label class="a11y-toggle" class:active={presetActive}>
			<input
				type="checkbox"
				checked={presetActive}
				aria-label={m.login_a11y_aria()}
				onchange={toggleMaxAccessibility}
			/>
			<span>{m.login_a11y_label()}</span>
		</label>
		{#if presetActive}
			<p class="a11y-active" role="status">{m.login_a11y_active()}</p>
		{/if}

		<div class="lang">
			<label class="lang-label" for="login-locale">{m.login_lang_label()}</label>
			<select id="login-locale" class="lang-select" onchange={onLocaleChange}>
				{#each localeOptions as opt (opt.value)}
					<option value={opt.value} selected={preferences.current.locale === opt.value}>
						{opt.label}
					</option>
				{/each}
			</select>
		</div>

		<p class="prefs-hint">{m.login_prefs_hint()}</p>
	</div>
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

	/* Secondary controls, kept compact and below the SSO button so the primary
	   login action stays dominant. */
	.controls {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	/* Prominent by design: someone reaching for "maximize accessibility" likely
	   struggles to read small text, so the toggle is large, full-width and high
	   contrast before the preset is even on. */
	.a11y-toggle {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		padding: 12px 14px;
		font-size: 1rem;
		font-weight: 600;
		color: var(--slate-100);
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 8px;
		cursor: pointer;
	}
	.a11y-toggle.active {
		border-color: var(--sky);
	}
	.a11y-toggle input {
		width: 22px;
		height: 22px;
		flex-shrink: 0;
		cursor: pointer;
	}
	/* accent-color themes the box but gives no focus ring on this dark theme —
	   highlight the enclosing control so the indicator is unmistakable. */
	.a11y-toggle:focus-within {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	.a11y-active {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.5;
		color: var(--slate-300);
	}

	.lang {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.lang-label {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--slate-200);
	}
	.lang-select {
		width: 100%;
		padding: 10px 12px;
		font: inherit;
		font-size: 0.9375rem;
		color: var(--slate-100);
		background: var(--space-800);
		border: 1px solid var(--space-700);
		border-radius: 8px;
		cursor: pointer;
	}
	.lang-select:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	.prefs-hint {
		margin: 0;
		font-size: 0.8125rem;
		line-height: 1.5;
		color: var(--slate-400);
	}
</style>
