// Floating-edge geometry. Adapted from the official @xyflow/svelte floating-edges
// example (https://svelteflow.dev/examples/edges/floating-edges): instead of
// anchoring an edge to a fixed handle, the endpoint floats to the point on each
// node's perimeter that faces the other node. The connection point migrates
// around the node as it (or its neighbour) is dragged.
//
// Our SystemNode exposes a `source` handle on all four sides (Left/Top/Right/
// Bottom) so `handleBounds.source` always has a handle for the chosen side; the
// `target` handles on Left/Top are there only so svelte-flow accepts incoming
// edges — the perimeter lookup below reads the `source` set.
import { Position, type InternalNode } from '@xyflow/svelte';

/**
 * The side of `nodeA` that faces `nodeB`, and the coordinate of that side's
 * handle. Picks Left/Right when the nodes are mostly side-by-side, Top/Bottom
 * when mostly stacked.
 */
function getParams(nodeA: InternalNode, nodeB: InternalNode): [number, number, Position] {
	const centerA = getNodeCenter(nodeA);
	const centerB = getNodeCenter(nodeB);

	const horizontalDiff = Math.abs(centerA.x - centerB.x);
	const verticalDiff = Math.abs(centerA.y - centerB.y);

	let position: Position;
	if (horizontalDiff > verticalDiff) {
		position = centerA.x > centerB.x ? Position.Left : Position.Right;
	} else {
		position = centerA.y > centerB.y ? Position.Top : Position.Bottom;
	}

	const [x, y] = getHandleCoordsByPosition(nodeA, position);
	return [x, y, position];
}

function getHandleCoordsByPosition(node: InternalNode, handlePosition: Position): [number, number] {
	// All sides carry a `source` handle (see SystemNode), so the chosen side's
	// handle always resolves.
	const handle = node.internals.handleBounds?.source?.find((h) => h.position === handlePosition);

	if (!handle?.width || !handle?.height) {
		return [0, 0];
	}

	let offsetX = handle.width / 2;
	let offsetY = handle.height / 2;

	// The handle position has a top-left origin; nudge to the correct edge so the
	// endpoint (and any marker) sits exactly on the node's perimeter.
	switch (handlePosition) {
		case Position.Left:
			offsetX = 0;
			break;
		case Position.Right:
			offsetX = handle.width;
			break;
		case Position.Top:
			offsetY = 0;
			break;
		case Position.Bottom:
			offsetY = handle.height;
			break;
	}

	const x = node.internals.positionAbsolute.x + handle.x + offsetX;
	const y = node.internals.positionAbsolute.y + handle.y + offsetY;

	return [x, y];
}

function getNodeCenter(node: InternalNode): { x: number; y: number } {
	return {
		x: node.internals.positionAbsolute.x + (node.measured.width ?? 0) / 2,
		y: node.internals.positionAbsolute.y + (node.measured.height ?? 0) / 2
	};
}

/** The endpoints + sides (sx, sy, tx, ty, sourcePos, targetPos) for a floating
 *  edge between two nodes — feed straight into `getBezierPath`. */
export function getEdgeParams(source: InternalNode, target: InternalNode) {
	const [sx, sy, sourcePos] = getParams(source, target);
	const [tx, ty, targetPos] = getParams(target, source);

	return { sx, sy, tx, ty, sourcePos, targetPos };
}
