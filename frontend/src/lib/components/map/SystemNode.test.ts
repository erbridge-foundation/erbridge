import { render, screen, cleanup } from '@testing-library/svelte';
import { afterEach, describe, it, expect } from 'vitest';
import Harness from './MapNodeHarness.test.svelte';
import type { Node } from '@xyflow/svelte';
import type { System } from '$lib/map/types';

const sys = (over: Partial<System> = {}): System => ({
	id: 'J100005',
	name: 'J100005',
	class: 'C5',
	statics: [{ wh_type: 'H900', dest: 'C5' }],
	...over
});

function node(
	data: { system: System; isRoot: boolean; isGhost: boolean },
	selected = false
): Node {
	return { id: data.system.id, type: 'system', position: { x: 0, y: 0 }, data, selected };
}

function renderNode(
	data: { system: System; isRoot: boolean; isGhost: boolean },
	selected = false
) {
	return render(Harness, { props: { nodes: [node(data, selected)] } });
}

afterEach(cleanup);

describe('SystemNode encoding (meaning never colour-only)', () => {
	it('renders the system class as TEXT', () => {
		renderNode({ system: sys({ class: 'C3' }), isRoot: false, isGhost: false });
		expect(screen.getByText('C3')).toBeInTheDocument();
	});

	it('renders security tiers (HS/LS/NS) as TEXT', () => {
		renderNode({ system: sys({ class: 'LS', statics: [] }), isRoot: false, isGhost: false });
		expect(screen.getByText('LS')).toBeInTheDocument();
	});

	it('renders a static by its DESTINATION class as TEXT, not the wormhole-type code', () => {
		renderNode({
			// A C5 system with a static leading to HS, whose wormhole type is B274.
			system: sys({ class: 'C5', statics: [{ wh_type: 'B274', dest: 'HS' }] }),
			isRoot: false,
			isGhost: false
		});
		// The static surfaces as its destination class (HS)...
		const statics = screen.getByLabelText('statics');
		expect(statics).toHaveTextContent('HS');
		// ...and the wormhole-type code is NOT shown (kept in the model for later).
		expect(screen.queryByText('B274')).toBeNull();
	});

	it('marks a root with a text badge, not colour alone', () => {
		const { container } = renderNode({ system: sys(), isRoot: true, isGhost: false });
		expect(screen.getByText('root')).toBeInTheDocument();
		expect(container.querySelector('.system-node.root')).not.toBeNull();
	});

	it('marks a ghost with a text badge and dashed (non-colour) styling', () => {
		const { container } = renderNode({ system: sys(), isRoot: false, isGhost: true });
		expect(screen.getByText('unconfirmed')).toBeInTheDocument();
		expect(container.querySelector('.system-node.ghost')).not.toBeNull();
	});

	it('is not marked selected when not selected', () => {
		const { container } = renderNode({ system: sys(), isRoot: false, isGhost: false });
		expect(container.querySelector('.system-node.selected')).toBeNull();
	});

	it('marks selection with the ring only — no size change or extra detail', () => {
		const { container } = renderNode({ system: sys(), isRoot: false, isGhost: false }, true);
		// Selection is the highlight ring; the node keeps its size/content (detail
		// lives in the sidebar intel) so it stays aligned with its edges.
		expect(container.querySelector('.system-node.selected')).not.toBeNull();
		expect(container.querySelector('.detail')).toBeNull();
		expect(screen.queryByText('Security')).toBeNull();
	});
});
