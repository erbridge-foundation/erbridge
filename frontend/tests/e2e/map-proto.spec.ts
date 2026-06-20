/**
 * E2E for the disposable map-canvas sandbox at /maps/_proto.
 *
 * The route is PUBLIC (no sign-in): it mounts MapCanvas against a static fixture
 * with no loader/auth. This spec proves the interaction model the prototype
 * exists to validate:
 *   - the position-less fixture renders as nodes + edges
 *   - dragging a node moves it (positions are session-only — ephemeral)
 *   - a reload RE-LAYS-OUT (a dragged position does NOT survive — no persistence)
 *   - "redo layout" reseeds positions from the root
 *   - "receive update" replays scripted SSE events, placing each incrementally:
 *     a new system is added, and a local ghost is confirmed into a real node
 *
 * Runs against the built SvelteKit app. No mock backend needed — the sandbox is
 * entirely client-side static.
 */

import { test, expect, type Page } from '@playwright/test';

/** A svelte-flow node by the system id it carries (node id == system id). */
function node(page: Page, id: string) {
	return page.locator(`.svelte-flow__node[data-id="${id}"]`);
}

async function nodePosition(page: Page, id: string): Promise<{ x: number; y: number }> {
	const box = await node(page, id).boundingBox();
	if (!box) throw new Error(`node ${id} has no bounding box`);
	return { x: box.x, y: box.y };
}

test.describe('/maps/_proto', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/maps/_proto');
		// Nodes render from the position-less fixture.
		await expect(node(page, 'Jita')).toBeVisible();
		// Sidebar sections start COLLAPSED on load; expand them so the tests that
		// exercise the in-section controls (layout, receive-update, colour-blind…)
		// can reach them. The collapsed-by-default behaviour is asserted by the
		// collapse-all/expand-all test, which reloads to observe the true default.
		await page.getByRole('button', { name: /expand all sections/i }).click();
	});

	test('renders nodes and edges from the position-less fixture', async ({ page }) => {
		// A spread of systems across classes/security renders.
		await expect(node(page, 'J100001')).toBeVisible();
		await expect(node(page, 'J100005')).toBeVisible();
		// Edges render (svelte-flow draws each as an .svelte-flow__edge).
		await expect(page.locator('.svelte-flow__edge').first()).toBeVisible();
		// Mass cue is text, not colour alone (the visible .mass label pill — scoped so
		// it doesn't match the hover-tooltip <title> which also contains "critical").
		// Mass labels default OFF now, so enable them via the prefs dialog first, then
		// the pill renders.
		await page.getByRole('button', { name: 'Map preferences' }).click();
		const prefs = page.getByRole('dialog', { name: 'Map preferences' });
		await prefs.getByLabel('Mass labels').check();
		await prefs.getByRole('button', { name: 'OK' }).click();
		await expect(prefs).toBeHidden();
		await expect(page.locator('.mass', { hasText: 'critical' }).first()).toBeVisible();
		// The imminent-closure connection surfaces its TTL state as sr-only text on the
		// centre label — meaning never relies on colour or motion alone (the mid-edge
		// glyph was dropped; this text now carries the precise four-state TTL).
		await expect(page.getByText('closure imminent').first()).toBeAttached();
		// Each connection carries a hover tooltip (<title> on the line's hit-path) with
		// its status — the tooltip moved off the deleted glyph onto the line itself.
		await expect(
			page.locator('.edge-hit title', { hasText: /mass/ }).first()
		).toBeAttached();
		// And it draws a breathing danger casing (the alert under-stroke). The casing
		// is a fill:none stroked path, which Playwright's visibility heuristic treats
		// as hidden — assert it is attached (present in the rendered edge) instead.
		await expect(
			page.locator('.svelte-flow__edge .edge-casing.halo-red').first()
		).toBeAttached();
	});

	test('the sidebar Signatures + Structures sections bind to the selected system', async ({
		page
	}) => {
		// Select J100003 — it carries a spread of scanned signatures.
		await node(page, 'J100003').click();
		const sigSection = page.getByRole('button', { name: /^Signatures/ });
		await expect(sigSection).toBeVisible();
		// Real fixture scan names show in the signature table (read off updated_at,
		// not the old hardcoded sample rows).
		await expect(page.getByText('Average Frontier Deposit')).toBeVisible();
		await expect(page.getByText('Ruined Rogue Drone Monument Site')).toBeVisible();
		// Wormhole sigs are colour-distinguished — the WH row carries the .wormhole
		// class (this is a wormholers' tool, so holes stand out from cosmic sites).
		await expect(page.locator('.sig-table tr.wormhole')).toHaveCount(1);

		// Select J100005 — it carries structures from two sources (scanner + overview),
		// the latter with a reinforcement timer.
		await node(page, 'J100005').click();
		await expect(page.getByText('J100005 - Home Fort')).toBeVisible();
		await expect(page.getByText('J100005 - Forward Op')).toBeVisible();
		await expect(page.getByText('reinforced')).toBeVisible();
		// The old hardcoded sample is gone.
		await expect(page.getByText('Fort Nightfall')).toHaveCount(0);
	});

	test('signatures can be added, renamed, and the add is uniqueness-checked', async ({ page }) => {
		await node(page, 'J100003').click();
		// The Signatures "+" opens an Add dialog; add a sig with a fresh id.
		await page.getByRole('button', { name: 'Add signature' }).click();
		const addDialog = page.getByRole('dialog', { name: 'Add signature' });
		await expect(addDialog).toBeVisible();
		await addDialog.getByLabel('Signature ID').fill('NEW-777');
		await addDialog.getByLabel('Name').fill('A Freshly Bookmarked Site');
		await addDialog.getByRole('button', { name: 'OK' }).click();
		await expect(page.getByText('A Freshly Bookmarked Site')).toBeVisible();

		// A duplicate id is rejected with an inline error (the existing ORE-330 row).
		await page.getByRole('button', { name: 'Add signature' }).click();
		await addDialog.getByLabel('Signature ID').fill('ORE-330');
		await addDialog.getByRole('button', { name: 'OK' }).click();
		await expect(addDialog.getByRole('alert')).toContainText('already exists');
		await addDialog.getByRole('button', { name: 'cancel' }).click();

		// Double-clicking a cosmic-site row opens Edit seeded with its values; rename it.
		await page.getByRole('row', { name: /Average Frontier Deposit/ }).dblclick();
		const editDialog = page.getByRole('dialog', { name: 'Edit signature' });
		await expect(editDialog.getByLabel('Signature ID')).toHaveValue('ORE-330');
		await editDialog.getByLabel('Name').fill('Rich Frontier Deposit');
		await editDialog.getByRole('button', { name: 'OK' }).click();
		await expect(page.getByText('Rich Frontier Deposit')).toBeVisible();
	});

	test('right-click offers Edit/Delete; delete is a stub; wormhole edit is gated', async ({
		page
	}) => {
		await node(page, 'J100003').click();

		// Right-clicking a sig row opens the Edit / Delete menu.
		const oreRow = page.getByRole('row', { name: /Average Frontier Deposit/ });
		await oreRow.click({ button: 'right' });
		const menu = page.getByRole('menu');
		await expect(menu.getByRole('menuitem', { name: 'Edit' })).toBeVisible();
		// Delete is a stub (real removal ties into the event/history model later): it
		// surfaces a "not implemented" notice; the menu closes and the row stays.
		await menu.getByRole('menuitem', { name: 'Delete' }).click();
		await expect(menu).toBeHidden();
		const notice = page.getByRole('dialog', { name: 'Not implemented' });
		await expect(notice).toBeVisible();
		await expect(notice).toContainText("isn't implemented yet");
		await notice.getByRole('button', { name: 'Close' }).click();
		await expect(notice).toBeHidden();
		await expect(oreRow).toBeVisible();

		// The wormhole row's edit is gated — a notice, not the fields.
		await page.getByRole('row', { name: /Unstable Wormhole/ }).dblclick();
		await expect(page.getByText(/Editing wormhole signatures isn't available yet/)).toBeVisible();
		await expect(page.getByLabel('Signature ID')).toHaveCount(0);
	});

	test('collapse-all / expand-all drive the sections; legend honours collapse-all only', async ({
		page
	}) => {
		const intel = page.getByRole('button', { name: 'System Intel' });
		const legendToggle = () => page.getByRole('button', { name: /show legend|hide legend/i });

		// Reload to undo the beforeEach expand and observe the true default: sections
		// start COLLAPSED, so the open sidebar shows a tidy list of headers.
		await page.reload();
		await expect(node(page, 'Jita')).toBeVisible();
		await expect(intel).toHaveAttribute('aria-expanded', 'false');

		// Expand all: every section opens.
		await page.getByRole('button', { name: /expand all sections/i }).click();
		await expect(intel).toHaveAttribute('aria-expanded', 'true');

		// Open the legend so we can watch collapse-all close it.
		await page.getByRole('button', { name: /show legend/i }).click();
		await expect(legendToggle()).toHaveAccessibleName(/hide legend/i);

		// Collapse all: every section closes AND the legend closes.
		await page.getByRole('button', { name: /collapse all sections/i }).click();
		await expect(intel).toHaveAttribute('aria-expanded', 'false');
		await expect(legendToggle()).toHaveAccessibleName(/show legend/i);

		// Expand all again: sections reopen, but the legend stays as the user left it
		// (collapsed) — expand-all does NOT touch the legend.
		await page.getByRole('button', { name: /expand all sections/i }).click();
		await expect(intel).toHaveAttribute('aria-expanded', 'true');
		await expect(legendToggle()).toHaveAccessibleName(/show legend/i);
	});

	test('locking the arrangement freezes the section + layout controls', async ({ page }) => {
		const intel = page.getByRole('button', { name: 'System Intel' });
		const flip = page.getByRole('button', { name: /move panel to the other side/i });
		const cog = page.getByRole('button', { name: 'Map preferences' });
		await expect(intel).toBeEnabled();
		await expect(flip).toBeEnabled();
		await expect(cog).toBeEnabled();

		await page.getByRole('button', { name: 'Lock arrangement', exact: true }).click();

		// Section toggles, flip, collapse/expand-all, and the preferences cog all
		// disable; the lock flips to an unlock affordance.
		await expect(intel).toBeDisabled();
		await expect(flip).toBeDisabled();
		await expect(cog).toBeDisabled();
		await expect(page.getByRole('button', { name: /collapse all sections/i })).toBeDisabled();
		await expect(page.getByRole('button', { name: /unlock arrangement/i })).toBeVisible();

		// Unlock restores them.
		await page.getByRole('button', { name: /unlock arrangement/i }).click();
		await expect(intel).toBeEnabled();
		await expect(flip).toBeEnabled();
		await expect(cog).toBeEnabled();
	});

	test('colour-blind palette toggle swaps the palette attribute AND recolours the legend', async ({
		page
	}) => {
		// The attribute sits on the STAGE (not just the flow) so the mass-hue swap
		// cascades to BOTH the canvas edges and the legend swatches in the sidebar.
		const stage = page.getByTestId('map-stage');
		await expect(stage).toHaveAttribute('data-edge-palette', 'standard');

		// Open the legend + read the fresh-mass swatch colour under the standard palette.
		await page.getByRole('button', { name: /show legend/i }).click();
		const freshSwatch = page.locator('.legend-body .line').first();
		const standardColour = await freshSwatch.evaluate(
			(el) => getComputedStyle(el).backgroundColor
		);

		// The toggle lives in the Map Canvas Tweaks sidebar section (expanded by the
		// beforeEach expand-all).
		await page.getByLabel('Colour-blind palette').check();
		await expect(stage).toHaveAttribute('data-edge-palette', 'colourblind');

		// The legend swatch must recolour in lock-step with the edges (the bug: it
		// previously sat outside the palette scope and stayed the standard green).
		const cbColour = await freshSwatch.evaluate((el) => getComputedStyle(el).backgroundColor);
		expect(cbColour).not.toBe(standardColour);
	});

	test('the signature-labels toggle shows/hides the per-end sig pills', async ({ page }) => {
		// On by default: the connection sig-id pills render (e.g. "ABC" from ABC-001).
		// EdgeLabels mount in svelte-flow's label-renderer portal, not inside the edge
		// <g>, so select the pill class page-wide.
		const pills = page.locator('.sig-endpoint');
		await expect(pills.first()).toBeVisible();
		const shown = await pills.count();
		expect(shown).toBeGreaterThan(0);

		// The label toggles now live in the Map Preferences dialog (cog on the tab bar).
		// Its blurred backdrop keeps the canvas visible behind, so the live change shows.
		await page.getByRole('button', { name: 'Map preferences' }).click();

		// Toggling it off withholds the ids → no pills render.
		await page.getByLabel('Signature labels').uncheck();
		await expect(pills).toHaveCount(0);

		// Back on restores them.
		await page.getByLabel('Signature labels').check();
		await expect(pills.first()).toBeVisible();
	});

	test('the preferences cog opens a dialog whose edits apply live, and closes', async ({
		page
	}) => {
		const dialog = page.getByRole('dialog', { name: 'Map preferences' });
		await expect(dialog).toBeHidden();

		// The cog on the tab bar opens the dialog.
		await page.getByRole('button', { name: 'Map preferences' }).click();
		await expect(dialog).toBeVisible();

		// A pref toggled in the dialog applies live to the canvas behind it (the blurred
		// backdrop keeps the canvas visible). Type labels default OFF → no wh-type spans;
		// checking it makes them appear.
		await expect(page.locator('.edge-label .wh-type')).toHaveCount(0);
		await dialog.getByLabel('Type labels').check();
		await expect(page.locator('.edge-label .wh-type').first()).toBeVisible();

		// Escape closes it and restores focus to the cog. Escape == Cancel, so the
		// Type-labels edit above is REVERTED — the wh-type spans vanish again.
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
		await expect(page.getByRole('button', { name: 'Map preferences' })).toBeFocused();
		await expect(page.locator('.edge-label .wh-type')).toHaveCount(0);
	});

	test('the animate-direction preference toggles the drift class on direction arrows', async ({
		page
	}) => {
		const dialog = page.getByRole('dialog', { name: 'Map preferences' });
		// Direction arrows render and are static (no drift) by default.
		await expect(page.locator('.dir-arrow').first()).toBeAttached();
		await expect(page.locator('.dir-glyph.drift')).toHaveCount(0);

		// Enable it in the dialog → the arrows gain the drift class (live preview).
		await page.getByRole('button', { name: 'Map preferences' }).click();
		await dialog.getByLabel('Animate direction').check();
		await expect(page.locator('.dir-glyph.drift').first()).toBeAttached();
		// Keep it (OK), then it persists after close.
		await dialog.getByRole('button', { name: 'OK' }).click();
		await expect(page.locator('.dir-glyph.drift').first()).toBeAttached();
	});

	test('OK keeps the live edits; Cancel reverts them', async ({ page }) => {
		const dialog = page.getByRole('dialog', { name: 'Map preferences' });
		const whType = page.locator('.edge-label .wh-type');

		// Type labels default OFF, so the canvas starts with no wh-type spans.
		await expect(whType).toHaveCount(0);

		// OK KEEPS: turn Type labels on, click OK → the change persists after close.
		await page.getByRole('button', { name: 'Map preferences' }).click();
		await dialog.getByLabel('Type labels').check();
		await expect(whType.first()).toBeVisible();
		await dialog.getByRole('button', { name: 'OK' }).click();
		await expect(dialog).toBeHidden();
		await expect(whType.first()).toBeVisible(); // kept

		// CANCEL REVERTS: reopen (now on), turn Type labels back off, then Cancel →
		// reverts to the on-open state (still on), not the mid-dialog edit.
		await page.getByRole('button', { name: 'Map preferences' }).click();
		await dialog.getByLabel('Type labels').uncheck();
		await expect(whType).toHaveCount(0);
		await dialog.getByRole('button', { name: 'cancel' }).click();
		await expect(dialog).toBeHidden();
		await expect(whType.first()).toBeVisible(); // reverted to the on-open (on) snapshot
	});

	test('the sidebar is resizable — dragging the gripper widens it', async ({ page }) => {
		const sidebar = page.locator('.sidebar-outer');
		const startW = (await sidebar.boundingBox())!.width;

		// The gripper sits on the inner edge; right-docked, so dragging LEFT widens.
		// Grab it NEAR THE TOP, clear of the collapse toggle which now sits over the
		// gripper at mid-height (the toggle owns that 24px band; the gripper is grabbable
		// along the rest of its height).
		const grip = page.getByRole('separator', { name: /resize panel/i });
		const gb = (await grip.boundingBox())!;
		const grabY = gb.y + 40;
		await page.mouse.move(gb.x + gb.width / 2, grabY);
		await page.mouse.down();
		await page.mouse.move(gb.x - 120, grabY, { steps: 8 });
		await page.mouse.up();

		const wider = (await sidebar.boundingBox())!.width;
		expect(wider).toBeGreaterThan(startW + 60);

		// The separator reports the new width for assistive tech.
		expect(Number(await grip.getAttribute('aria-valuenow'))).toBeGreaterThan(startW);
	});

	test('edges float — the connection path follows a node as it is dragged', async ({ page }) => {
		// The Jita→J100001 edge path. Floating edges anchor to each node's perimeter,
		// so dragging an endpoint recomputes the bezier path's `d`.
		const edgePath = page.locator(
			'.svelte-flow__edge[data-id="c-jita-j1"] path.svelte-flow__edge-path'
		);
		const before = await edgePath.getAttribute('d');
		expect(before).toBeTruthy();

		// Drag J100001 to a new spot.
		const box = await node(page, 'J100001').boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 200, box.y + 150, { steps: 8 });
		await page.mouse.up();

		// The path changed — the endpoint floated to follow the node.
		const after = await edgePath.getAttribute('d');
		expect(after).not.toBe(before);
	});

	test('a reload re-lays-out — a dragged position is ephemeral, not persisted', async ({ page }) => {
		const before = await nodePosition(page, 'J100001');

		// Drag J100001 a clear distance.
		const handle = node(page, 'J100001');
		const box = await handle.boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 180, box.y + 140, { steps: 8 });
		await page.mouse.up();

		const afterDrag = await nodePosition(page, 'J100001');
		expect(Math.abs(afterDrag.x - before.x) + Math.abs(afterDrag.y - before.y)).toBeGreaterThan(40);

		// Reload: with no persistence (Fork 1 reversed), the map re-lays-out from the
		// fixture, so the node returns to its seed position — NOT the dragged spot.
		await page.reload();
		await expect(node(page, 'J100001')).toBeVisible();

		const afterReload = await nodePosition(page, 'J100001');
		// Back to (near) the original seeded position; clearly NOT the dragged spot.
		expect(Math.abs(afterReload.x - before.x)).toBeLessThan(30);
		expect(Math.abs(afterReload.y - before.y)).toBeLessThan(30);
		expect(Math.abs(afterReload.x - afterDrag.x) + Math.abs(afterReload.y - afterDrag.y)).toBeGreaterThan(40);
	});

	test('selecting a style then Apply layout reseeds node positions', async ({ page }) => {
		// Drag a node well away first.
		const box = await node(page, 'J100001').boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 220, box.y + 160, { steps: 8 });
		await page.mouse.up();
		const dragged = await nodePosition(page, 'J100001');

		// The layout style picker is the tab-bar split-button now. Open its caret
		// dropdown and pick the top→bottom style; with auto OFF this only records the
		// choice (the dragged node must NOT move yet).
		await page.getByRole('button', { name: 'Choose layout style' }).click();
		await page.getByRole('menuitemradio', { name: /top.*bottom/i }).click();
		const afterSelect = await nodePosition(page, 'J100001');
		expect(Math.abs(afterSelect.x - dragged.x) + Math.abs(afterSelect.y - dragged.y)).toBeLessThan(2);

		// Now the action half (apply now) reflows in the selected style → the node
		// leaves the dragged spot.
		await page.getByRole('button', { name: /apply layout: top . bottom/i }).click();
		await expect(node(page, 'J100001')).toBeVisible();
		const reseeded = await nodePosition(page, 'J100001');
		expect(Math.abs(reseeded.x - dragged.x) + Math.abs(reseeded.y - dragged.y)).toBeGreaterThan(30);
	});

	test('auto-layout reflows the whole map when an event arrives', async ({ page }) => {
		// Drag a node away; with auto-layout ON, the next received event reflows the
		// whole map and discards the drag.
		const box = await node(page, 'J100001').boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 220, box.y + 160, { steps: 8 });
		await page.mouse.up();
		const dragged = await nodePosition(page, 'J100001');

		// Auto-layout toggle is in the Map Preferences dialog; enable it then confirm
		// with OK (keep — Escape would Cancel/revert the toggle).
		await page.getByRole('button', { name: 'Map preferences' }).click();
		await page.getByLabel(/auto-layout on changes/i).check();
		await page.getByRole('dialog', { name: 'Map preferences' }).getByRole('button', { name: 'OK' }).click();
		// Apply one scripted SSE event.
		await page.getByRole('button', { name: /receive update/i }).click();

		// The dragged node was reflowed back onto the grid (auto = machine-owned).
		await expect
			.poll(async () => {
				const p = await nodePosition(page, 'J100001');
				return Math.abs(p.x - dragged.x) + Math.abs(p.y - dragged.y);
			})
			.toBeGreaterThan(30);
	});

	test('the legend toggles open/closed and keys the encoding', async ({ page }) => {
		// Collapsed by default: only the header shows, body is absent.
		const open = page.getByRole('button', { name: /show legend/i });
		await expect(open).toBeVisible();
		await expect(page.getByRole('heading', { name: /connection mass/i })).toHaveCount(0);

		// Expand: the encoding groups appear (mass / ttl / systems / other).
		await open.click();
		const legend = page.getByTestId('map-legend');
		await expect(legend.getByRole('heading', { name: /connection mass/i })).toBeVisible();
		await expect(legend.getByRole('heading', { name: /time to live/i })).toBeVisible();
		// Meaning is text beside each swatch (a11y rule): the mass labels are present.
		// Scope to the mass group's row list (the TTL group no longer has a "stable"
		// row — stable = no glow is the implicit default, so it gets no legend entry).
		const massRows = legend.locator('.rows').first();
		await expect(massRows.getByText('stable', { exact: true })).toBeVisible();

		// Collapse again.
		await page.getByRole('button', { name: /hide legend/i }).click();
		await expect(page.getByRole('heading', { name: /connection mass/i })).toHaveCount(0);
	});

	test('each tab is its own placement snowflake — a drag stays with its tab', async ({ page }) => {
		// J100001 renders in both the Home tab (it is Home's root) and the wildcard
		// `*` tab (which shows every system), so it is the shared node that proves a
		// tab does NOT inherit another tab's arrangement.
		const homeSpot = await nodePosition(page, 'J100001');

		// Drag it well away on the Home tab.
		const box = await node(page, 'J100001').boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 200, box.y + 150, { steps: 8 });
		await page.mouse.up();
		const homeDragged = await nodePosition(page, 'J100001');
		expect(
			Math.abs(homeDragged.x - homeSpot.x) + Math.abs(homeDragged.y - homeSpot.y)
		).toBeGreaterThan(40);

		// Switch to the wildcard tab: J100001 must take that tab's OWN seed, not the
		// Home drag — the two tabs are independent snowflakes.
		await page.getByRole('button', { name: '*', exact: true }).click();
		await expect(node(page, 'J100001')).toBeVisible();
		const wildSpot = await nodePosition(page, 'J100001');
		expect(
			Math.abs(wildSpot.x - homeDragged.x) + Math.abs(wildSpot.y - homeDragged.y)
		).toBeGreaterThan(40);

		// Switch back to Home: the node returns to where it was dragged THERE — the
		// tab remembered its own arrangement across the round-trip.
		await page.getByRole('button', { name: 'Home', exact: true }).click();
		await expect(node(page, 'J100001')).toBeVisible();
		const homeReturn = await nodePosition(page, 'J100001');
		expect(Math.abs(homeReturn.x - homeDragged.x)).toBeLessThan(30);
		expect(Math.abs(homeReturn.y - homeDragged.y)).toBeLessThan(30);
	});

	test('a scanned-but-unjumped wormhole sig renders a faint dangling stub node', async ({
		page
	}) => {
		// The Home tab has no dangling holes; the wildcard `*` tab renders every system
		// (incl. the DEEP systems whose wormhole scans no connection reaches yet), so a
		// dangling stub appears there.
		await page.getByRole('button', { name: '*', exact: true }).click();

		// The stub renders with the minimal dangling style + a `?` glyph. Its node id is
		// namespaced (`dangling:<system>:<sig>`), so select by the stub class.
		const stub = page.locator('.svelte-flow__node .system-node.dangling').first();
		await expect(stub).toBeVisible();
		await expect(stub.locator('.unknown', { hasText: '?' })).toBeVisible();
		// A known wormhole type infers a destination class (e.g. R474 → C6); at least one
		// stub on the map should carry an inferred dest badge.
		await expect(
			page.locator('.system-node.dangling .badge.class').first()
		).toBeVisible();
	});

	test('the node-spacing preference reflows the layout (spreads siblings apart)', async ({
		page
	}) => {
		// A node down a sibling fan on the Home chain, so a cross-axis spacing change
		// visibly moves it (tidy-tree packs the cross axis, so a deeper fan node shifts
		// well under a spacing bump — J100004 moves ~225px from default to max).
		const target = 'J100004';
		await expect(node(page, target)).toBeVisible();
		const before = await nodePosition(page, target);

		// Open prefs and push node spacing to the maximum → the active tab reflows with
		// wider cross-axis gaps, so the node takes a new seed position.
		await page.getByRole('button', { name: 'Map preferences' }).click();
		const dialog = page.getByRole('dialog', { name: 'Map preferences' });
		const spacing = dialog.getByRole('slider', { name: 'Node spacing' });
		await spacing.focus();
		// Drive it to the max with End (the slider's keyboard support), then close.
		await page.keyboard.press('End');
		await dialog.getByRole('button', { name: 'OK' }).click();
		await expect(dialog).toBeHidden();

		const after = await nodePosition(page, target);
		expect(Math.abs(after.x - before.x) + Math.abs(after.y - before.y)).toBeGreaterThan(20);
	});

	test('receive update replays scripted SSE events incrementally', async ({ page }) => {
		const receive = page.getByRole('button', { name: /receive update/i });

		// The ghost J199999 is present (local state) and styled as unconfirmed.
		await expect(node(page, 'J199999')).toBeVisible();
		await expect(page.getByText('unconfirmed')).toBeVisible();

		// Event 1: a brand-new system (J100008) is added, reached from J100006.
		await receive.click();
		await expect(node(page, 'J100008')).toBeVisible();
		// The ghost is still unconfirmed — this event didn't touch it.
		await expect(page.getByText('unconfirmed')).toBeVisible();

		// Event 2: J199999 is confirmed as server truth (gains a connection). It
		// drops from local state, so it renders once, without the ghost badge.
		await receive.click();
		await expect(node(page, 'J199999')).toHaveCount(1);
		await expect(page.getByText('unconfirmed')).toHaveCount(0);

		// Event 3: EC-P8R departs the graph and disappears.
		await expect(node(page, 'EC-P8R')).toBeVisible();
		await receive.click();
		await expect(node(page, 'EC-P8R')).toHaveCount(0);
	});
});
