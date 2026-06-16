// Node-collision relaxation, adapted from the official @xyflow/svelte "Node
// Collisions" example (https://svelteflow.dev/examples/misc/collisions). Pushes
// overlapping nodes apart along the smallest-overlap axis, iterating to
// convergence. Used on drag-stop and after an SSE add (where the freshly placed
// node may overlap its neighbours). It moves whatever it must to clear overlaps,
// so existing nodes can shift to make room — that's the intended behaviour.
import type { Node } from '@xyflow/svelte';

export type CollisionAlgorithmOptions = {
	maxIterations: number;
	overlapThreshold: number;
	margin: number;
};

type Box = {
	x: number;
	y: number;
	width: number;
	height: number;
	moved: boolean;
	node: Node;
};

function getBoxesFromNodes(nodes: Node[], margin = 0): Box[] {
	const boxes: Box[] = new Array(nodes.length);
	for (let i = 0; i < nodes.length; i++) {
		const node = nodes[i];
		boxes[i] = {
			x: node.position.x - margin,
			y: node.position.y - margin,
			width: (node.width ?? node.measured?.width ?? 0) + margin * 2,
			height: (node.height ?? node.measured?.height ?? 0) + margin * 2,
			node,
			moved: false
		};
	}
	return boxes;
}

export function resolveCollisions(
	nodes: Node[],
	{ maxIterations = 50, overlapThreshold = 0.5, margin = 0 }: CollisionAlgorithmOptions
): Node[] {
	const boxes = getBoxesFromNodes(nodes, margin);

	for (let iter = 0; iter <= maxIterations; iter++) {
		let moved = false;

		for (let i = 0; i < boxes.length; i++) {
			for (let j = i + 1; j < boxes.length; j++) {
				const A = boxes[i];
				const B = boxes[j];

				const centerAX = A.x + A.width * 0.5;
				const centerAY = A.y + A.height * 0.5;
				const centerBX = B.x + B.width * 0.5;
				const centerBY = B.y + B.height * 0.5;

				const dx = centerAX - centerBX;
				const dy = centerAY - centerBY;

				// Overlap along each axis.
				const px = (A.width + B.width) * 0.5 - Math.abs(dx);
				const py = (A.height + B.height) * 0.5 - Math.abs(dy);

				if (px > overlapThreshold && py > overlapThreshold) {
					A.moved = B.moved = moved = true;
					// Resolve along the smallest-overlap axis.
					if (px < py) {
						const sx = dx > 0 ? 1 : -1;
						const moveAmount = (px / 2) * sx;
						A.x += moveAmount;
						B.x -= moveAmount;
					} else {
						const sy = dy > 0 ? 1 : -1;
						const moveAmount = (py / 2) * sy;
						A.y += moveAmount;
						B.y -= moveAmount;
					}
				}
			}
		}

		if (!moved) break; // converged
	}

	return boxes.map((box) =>
		box.moved ? { ...box.node, position: { x: box.x + margin, y: box.y + margin } } : box.node
	);
}
