import { render, screen, cleanup } from '@testing-library/svelte';
import { afterEach, describe, it, expect } from 'vitest';
import Harness from './MapNodeHarness.test.svelte';
import type { Node } from '@xyflow/svelte';
import type { System } from '$lib/map/types';

const sys = (over: Partial<System> = {}): System => ({
	id: 'J100005',
	name: 'J100005',
	class: 'C5',
	statics: [{ code: 'C5a', dest: 'C5' }],
	...over
});

function node(data: { system: System; isRoot: boolean; isGhost: boolean }): Node {
	return { id: data.system.id, type: 'system', position: { x: 0, y: 0 }, data };
}

function renderNode(data: { system: System; isRoot: boolean; isGhost: boolean }) {
	return render(Harness, { props: { nodes: [node(data)] } });
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

	it('renders static codes as TEXT', () => {
		renderNode({
			system: sys({ statics: [{ code: 'HSa', dest: 'HS' }] }),
			isRoot: false,
			isGhost: false
		});
		expect(screen.getByText('HSa')).toBeInTheDocument();
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
});
