import type { Action } from 'svelte/action';
import { registerBelowKeyboard } from 'tauri-plugin-virtual-keyboard';

/**
 * Render an element below the keyboard — the counterpart to `renderAboveKeyboard`.
 * The bits above the keyboard (composer bar, message list) glide up; this element
 * is revealed in the freed space, standing in for the keyboard.
 *
 * The element must pin itself to the viewport bottom and must not sit inside a
 * `renderAboveKeyboard` node — see `MediaPanel`.
 */
export const renderBelowKeyboard: Action<HTMLElement, { open: boolean }> = (
	node,
	params,
) => {
	const surface = registerBelowKeyboard(node);
	surface.setOpen(params.open);
	return {
		update(next) {
			surface.setOpen(next.open);
		},
		destroy() {
			surface.destroy();
		},
	};
};
