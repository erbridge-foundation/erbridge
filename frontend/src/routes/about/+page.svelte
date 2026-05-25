<script lang="ts">
	import type { PageData } from "./$types";

	let { data }: { data: PageData } = $props();

	const uiVersion = import.meta.env.PUBLIC_UI_VERSION;

	// Curated by the maintainers — editing this list is a code change (design §7).
	const acknowledgements = [
		{
			name: "Tripwire",
			href: "https://tripwiremap.app/",
			desc: "the wormhole-mapping reference for a generation of W-space pilots; pioneered the chain-aware signature workflow.",
		},
		{
			name: "Wanderer",
			href: "https://wanderer.ltd/",
			desc: "modern, open-source, multi-character mapping with strong real-time semantics.",
		},
		{
			name: "Anoikis.info",
			href: "https://anoikis.info/",
			desc: "the institutional encyclopedia of W-space; the static-info source the community has trusted for years.",
		},
		{
			name: "EVE Scout",
			href: "https://www.eve-scout.com/",
			desc: "the Signal Cartel community effort that scouts and publicly shares the Thera and Turnur connections — open wormhole intel as a free service.",
		},
	];
</script>

<svelte:head>
	<title>About · E-R Bridge</title>
</svelte:head>

<main class="about">
	<header class="about-header">
		<div class="mark">
			<svg
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
				<path
					d="M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M18.4 5.6l-2.8 2.8M8.4 15.6l-2.8 2.8"
				></path>
			</svg>
			<span class="title">E-R BRIDGE</span>
		</div>
		<p class="tagline">Wormhole Mapper for EVE Online</p>
	</header>

	<section class="section">
		<h2 class="section-label">Versions</h2>
		<div class="version-row">
			<span class="label">UI version</span>
			<span class="value">{uiVersion}</span>
		</div>
		<div class="version-row">
			<span class="label">API version</span>
			{#if data.health}
				<span class="value"
					>{data.health.version} ·
					<span class="commit">{data.health.commit}</span></span
				>
			{:else}
				<span class="value unreachable">API: unreachable</span>
			{/if}
		</div>
	</section>

	<section class="section source-link">
		<h2 class="section-label">Source code</h2>
		<a
			href="https://github.com/erbridge-foundation/erbridge"
			target="_blank"
			rel="noopener noreferrer">Source on GitHub →</a
		>
	</section>

	<section class="section">
		<h2 class="section-label">Legal</h2>
		<p class="legal">
			EVE Online and the EVE logo are the registered trademarks of CCP hf. All
			rights are reserved worldwide. All other trademarks are the property of
			their respective owners. EVE Online, the EVE logo, EVE and all associated
			logos and designs are the intellectual property of CCP hf. All artwork,
			screenshots, characters, vehicles, storylines, world facts or other
			recognizable features of the intellectual property relating to these
			trademarks are likewise the intellectual property of CCP hf. CCP hf. has
			granted permission to E-R Bridge to use EVE Online and all associated
			logos and designs for promotional and information purposes on its website
			but does not endorse, and is not in any way affiliated with, E-R Bridge.
			CCP is in no way responsible for the content on or functioning of this
			website, nor can it be liable for any damage arising from the use of this
			website.
		</p>
	</section>

	<section class="section">
		<h2 class="section-label">Acknowledgements</h2>
		<ul class="ack-list">
			{#each acknowledgements as ack (ack.href)}
				<li class="ack">
					<a
						class="ack-card"
						href={ack.href}
						target="_blank"
						rel="noopener noreferrer"
					>
						<span class="ack-name">{ack.name}</span>
						<span class="ack-desc">{ack.desc}</span>
					</a>
				</li>
			{/each}
		</ul>
	</section>
</main>

<style>
	.about {
		/* The app shell is a fixed-height (100vh) column with overflow:hidden, so
		   this page must be its own scroll region or its bottom is clipped with no
		   scrollbar. `flex: 1; min-height: 0` lets it fill the remaining height and
		   shrink enough to scroll; `overflow-y: auto` provides the scrollbar. The
		   content column stays centred via the max-width + auto margins (the flex
		   box itself is full-width). Matches the characters page's content width. */
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		width: 100%;
		max-width: 960px;
		margin: 0 auto;
		padding: 48px 24px 64px;
	}

	.about-header {
		margin-bottom: 40px;
	}
	.mark {
		display: flex;
		align-items: center;
		gap: 12px;
		color: var(--sky);
	}
	.title {
		font-size: 1.25rem;
		font-weight: 600;
		letter-spacing: 0.18em;
		color: var(--slate-100);
	}
	.tagline {
		margin-top: 8px;
		color: var(--slate-400);
		font-size: 0.8125rem;
	}

	.section {
		margin-bottom: 32px;
	}
	.section-label {
		font-size: 0.6875rem;
		font-weight: 600;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		color: var(--slate-500);
		margin: 0 0 12px;
	}

	.version-row {
		display: flex;
		justify-content: space-between;
		gap: 16px;
		padding: 6px 0;
		border-bottom: 1px solid var(--space-800);
	}
	.version-row .label {
		color: var(--slate-400);
	}
	.version-row .value {
		color: var(--slate-200);
	}
	.version-row .value .commit {
		color: var(--slate-500);
	}
	.version-row .value.unreachable {
		color: var(--amber);
	}

	.source-link a {
		color: var(--sky);
		text-decoration: none;
	}
	.source-link a:hover {
		text-decoration: underline;
	}

	.legal {
		color: var(--slate-400);
		font-size: 0.75rem;
		line-height: 1.7;
		margin: 0;
	}

	.ack-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 16px;
	}
	@media (max-width: 600px) {
		.ack-list {
			grid-template-columns: 1fr;
		}
	}

	.ack {
		display: flex;
	}

	/* The whole card is the link — a larger, clearer target (and it respects the
	   large-targets accessibility preference, which sizes `a` elements). */
	.ack-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		padding: 16px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		text-decoration: none;
	}
	.ack-card:hover {
		background: var(--space-800);
		border-color: var(--space-600);
	}
	.ack-card:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: 2px;
	}

	.ack-name {
		font-weight: 600;
		color: var(--sky);
	}
	.ack-desc {
		color: var(--slate-400);
		font-size: 0.75rem;
		line-height: 1.5;
	}
</style>
