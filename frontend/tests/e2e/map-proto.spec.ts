/**
 * E2E for the disposable map-canvas sandbox at /maps/_proto.
 *
 * The route is PUBLIC (no sign-in): it mounts MapCanvas against a static fixture
 * with no loader/auth. This spec proves the interaction model the prototype
 * exists to validate:
 *   - the position-less fixture renders as nodes + edges
 *   - dragging a node moves it (positions are session-only — ephemeral)
 *   - a reload RE-LAYS-OUT (a dragged position does NOT survive — no persistence)
 *   - "redo layout" reseeds positions from the roots
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
	});

	test('renders nodes and edges from the position-less fixture', async ({ page }) => {
		// A spread of systems across classes/security renders.
		await expect(node(page, 'J100001')).toBeVisible();
		await expect(node(page, 'J100005')).toBeVisible();
		// Edges render (svelte-flow draws each as an .svelte-flow__edge).
		await expect(page.locator('.svelte-flow__edge').first()).toBeVisible();
		// Mass cue is text, not colour alone.
		await expect(page.getByText('critical').first()).toBeVisible();
		// The imminent-closure connection surfaces its TTL state as text (the SVG
		// glyph's accessible name) — meaning never relies on colour or shape alone.
		await expect(page.getByText('closure imminent').first()).toBeVisible();
		// And it draws a breathing danger casing (the alert under-stroke). The casing
		// is a fill:none stroked path, which Playwright's visibility heuristic treats
		// as hidden — assert it is attached (present in the rendered edge) instead.
		await expect(
			page.locator('.svelte-flow__edge .edge-casing.halo-red').first()
		).toBeAttached();
	});

	test('colour-blind palette toggle swaps the canvas palette attribute', async ({ page }) => {
		const flow = page.getByTestId('map-flow');
		await expect(flow).toHaveAttribute('data-edge-palette', 'standard');
		// The toggle lives in the Map Canvas Tweaks sidebar section (open by default).
		await page.getByLabel('Colour-blind palette').check();
		await expect(flow).toHaveAttribute('data-edge-palette', 'colourblind');
	});

	test('the sidebar is resizable — dragging the gripper widens it', async ({ page }) => {
		const sidebar = page.locator('.sidebar-outer');
		const startW = (await sidebar.boundingBox())!.width;

		// The gripper sits on the inner edge; right-docked, so dragging LEFT widens.
		const grip = page.getByRole('separator', { name: /resize panel/i });
		const gb = (await grip.boundingBox())!;
		await page.mouse.move(gb.x + gb.width / 2, gb.y + gb.height / 2);
		await page.mouse.down();
		await page.mouse.move(gb.x - 120, gb.y + gb.height / 2, { steps: 8 });
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

	test('redo layout reseeds node positions', async ({ page }) => {
		// Drag a node well away first.
		const box = await node(page, 'J100001').boundingBox();
		if (!box) throw new Error('no box');
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await page.mouse.move(box.x + 220, box.y + 160, { steps: 8 });
		await page.mouse.up();
		const dragged = await nodePosition(page, 'J100001');

		// Open the layout slide-out and pick top→bottom.
		await page.getByRole('button', { name: /redo layout/i }).click();
		await page.getByRole('button', { name: /top.*bottom/i }).click();

		await expect(node(page, 'J100001')).toBeVisible();
		const reseeded = await nodePosition(page, 'J100001');
		// The node moved off the dragged spot (back onto the BFS grid).
		expect(Math.abs(reseeded.x - dragged.x) + Math.abs(reseeded.y - dragged.y)).toBeGreaterThan(30);
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
		await expect(legend.getByText('fresh', { exact: true })).toBeVisible();

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
