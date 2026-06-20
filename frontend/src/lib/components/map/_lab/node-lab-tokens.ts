// Shared class / flag colour + glyph tokens for the NODE LAB variant components.
// DISPOSABLE — extracted so the wireframe variants don't each re-declare the same maps
// (they mirror SystemNode.svelte's tokens). When a node variant is chosen and promoted,
// this folder is deleted and the winning variant folds back into SystemNode.
import { m } from '$lib/paraglide/messages';
import type { SystemClass, SystemFlag } from '$lib/map/types';

/** Class → decorative colour token (the class TEXT is the real signal; colour decorates). */
export const classColour: Record<SystemClass, string> = {
	C1: 'var(--c1)',
	C2: 'var(--c2)',
	C3: 'var(--c3)',
	C4: 'var(--c4)',
	C5: 'var(--c5)',
	C6: 'var(--c6)',
	HS: 'var(--hs)',
	LS: 'var(--ls)',
	NS: 'var(--ns)',
	P: 'var(--pochven)',
	D: 'var(--drifter)'
};

/** Intel flag → distinct glyph SHAPE (carries meaning in greyscale; colour decorates). */
export const flagGlyph: Record<SystemFlag, string> = {
	target: '◎',
	warning: '⚠',
	friendly: '✚',
	'looking-for': '⌕'
};

export const flagColour: Record<SystemFlag, string> = {
	target: 'var(--violet)',
	warning: 'var(--alert-warning)',
	friendly: 'var(--emerald)',
	'looking-for': 'var(--sky)'
};

export const flagLabel: Record<SystemFlag, string> = {
	target: m.map_proto_flag_target(),
	warning: m.map_proto_flag_warning(),
	friendly: m.map_proto_flag_friendly(),
	'looking-for': m.map_proto_flag_looking_for()
};
