<script lang="ts">
	// A split-button for the map's auto-layout STYLE, living on the tab bar next to
	// the preferences cog. Two halves:
	//   - ACTION (left): an icon button whose face is the org-chart glyph oriented to
	//     the CURRENT style; clicking it applies/reflows the layout now.
	//   - CARET (right): a ▾ that opens a small dropdown of the four styles (each an
	//     oriented org-chart icon); picking one selects it (becoming the action face).
	// Reuses the app's UserChip/UserMenu dropdown pattern (anchor + document-click +
	// Escape). Session-only prototype control — no persistence here.
	import { m } from '$lib/paraglide/messages';
	import type { LayoutDirection } from '$lib/map/types';

	let {
		layoutDir,
		onSelect,
		onApply,
		disabled = false
	}: {
		/** The currently-selected layout style — drives the action button's icon. */
		layoutDir: LayoutDirection;
		/** Choose a style (the parent reflows immediately if auto-layout is on). */
		onSelect: (dir: LayoutDirection) => void;
		/** Apply/reflow the layout now in the current style. */
		onApply: () => void;
		disabled?: boolean;
	} = $props();

	const styles: { dir: LayoutDirection; label: () => string }[] = [
		{ dir: 'LR', label: m.map_proto_layout_lr },
		{ dir: 'RL', label: m.map_proto_layout_rl },
		{ dir: 'TB', label: m.map_proto_layout_tb },
		{ dir: 'BT', label: m.map_proto_layout_bt }
	];
	function labelFor(dir: LayoutDirection): string {
		return styles.find((s) => s.dir === dir)!.label();
	}

	// The base org-chart icon is drawn parent-TOP / children-BOTTOM (the natural
	// hierarchy). Every other direction is the same glyph rotated, so a single SVG
	// covers all four — the rotation makes the parent point the "from" way.
	const rotation: Record<LayoutDirection, number> = {
		TB: 0,
		BT: 180,
		LR: -90, // parent swings to the left, children to the right
		RL: 90
	};

	let open = $state(false);
	let anchor: HTMLDivElement;

	function close() {
		open = false;
	}
	function toggle() {
		if (disabled) return;
		open = !open;
	}
	function pick(dir: LayoutDirection) {
		onSelect(dir);
		close();
	}

	function onDocumentClick(e: MouseEvent) {
		if (anchor && !anchor.contains(e.target as Node)) close();
	}
	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') close();
	}
	$effect(() => {
		if (open) {
			document.addEventListener('click', onDocumentClick);
			document.addEventListener('keydown', onKeydown);
		}
		return () => {
			document.removeEventListener('click', onDocumentClick);
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

{#snippet orgChart(dir: LayoutDirection)}
	<!-- One parent node + TWO children + connectors, rotated per direction. (Two
	     children read more clearly than three at icon size, esp. when rotated.) -->
	<svg
		class="org-icon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="1.8"
		stroke-linecap="round"
		stroke-linejoin="round"
		style:transform={`rotate(${rotation[dir]}deg)`}
		aria-hidden="true"
	>
		<!-- parent (top centre) -->
		<rect x="9.5" y="2.5" width="5" height="4" rx="1" />
		<!-- two children (bottom row, flanking centre) -->
		<rect x="3.5" y="17.5" width="5" height="4" rx="1" />
		<rect x="15.5" y="17.5" width="5" height="4" rx="1" />
		<!-- connectors: parent down to a bus, bus across, drops to each child -->
		<path d="M12 6.5V11" />
		<path d="M6 17.5V14H18V17.5" />
	</svg>
{/snippet}

<div class="layout-menu" bind:this={anchor}>
	<!-- Action: apply the current style now. Face = the current style's icon. -->
	<button
		type="button"
		class="apply-btn"
		{disabled}
		aria-label={m.map_proto_layout_apply({ style: labelFor(layoutDir) })}
		title={m.map_proto_layout_apply({ style: labelFor(layoutDir) })}
		onclick={onApply}
	>
		{@render orgChart(layoutDir)}
	</button>

	<!-- Caret: open the style dropdown. -->
	<button
		type="button"
		class="caret-btn"
		{disabled}
		aria-haspopup="menu"
		aria-expanded={open}
		aria-label={m.map_proto_layout_pick()}
		title={m.map_proto_layout_pick()}
		onclick={toggle}
	>
		<svg
			class="caret"
			class:flipped={open}
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			aria-hidden="true"
		>
			<polyline points="6 9 12 15 18 9" />
		</svg>
	</button>

	{#if open}
		<div class="menu" role="menu">
			{#each styles as style (style.dir)}
				<button
					type="button"
					class="menu-item"
					role="menuitemradio"
					aria-checked={layoutDir === style.dir}
					onclick={() => pick(style.dir)}
				>
					{@render orgChart(style.dir)}
					<span class="menu-label">{style.label()}</span>
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.layout-menu {
		position: relative;
		display: inline-flex;
		align-items: stretch;
	}
	.apply-btn,
	.caret-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		background: none;
		border: 1px solid transparent;
		color: var(--slate-400);
		cursor: pointer;
		padding: 0 6px;
	}
	.apply-btn {
		border-radius: 4px 0 0 4px;
	}
	.caret-btn {
		border-radius: 0 4px 4px 0;
		border-left-color: var(--space-700);
		padding: 0 2px;
	}
	.apply-btn:hover:not(:disabled),
	.caret-btn:hover:not(:disabled) {
		color: var(--slate-100);
		background: var(--space-800);
	}
	.apply-btn:focus-visible,
	.caret-btn:focus-visible,
	.menu-item:focus-visible {
		outline: 2px solid var(--sky);
		outline-offset: -2px;
	}
	.apply-btn:disabled,
	.caret-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
	.org-icon {
		width: 18px;
		height: 18px;
	}
	.caret {
		width: 12px;
		height: 12px;
		transition: transform 0.15s ease;
	}
	.caret.flipped {
		transform: rotate(180deg);
	}

	.menu {
		position: absolute;
		top: calc(100% + 6px);
		right: 0;
		min-width: 170px;
		background: var(--space-900);
		border: 1px solid var(--space-700);
		border-radius: 6px;
		box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
		padding: 4px;
		z-index: 50;
	}
	.menu-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 6px 10px;
		background: none;
		border: 0;
		border-radius: 4px;
		color: var(--slate-200);
		font: inherit;
		font-size: 0.75rem;
		text-align: left;
		cursor: pointer;
	}
	.menu-item:hover {
		background: var(--space-700);
	}
	/* The active style: sky tint + brighter text. aria-checked is the source of
	   truth (not colour alone), exposed to AT via menuitemradio. */
	.menu-item[aria-checked='true'] {
		color: var(--slate-100);
		background: var(--space-800);
	}
	.menu-item[aria-checked='true'] .org-icon {
		color: var(--sky);
	}
</style>
