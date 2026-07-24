import type { Action } from 'svelte/action';
import { registerBelowKeyboard } from 'tauri-plugin-virtual-keyboard';

/**
 * Render an element below the keyboard — the counterpart to `renderAboveKeyboard`.
 * The bits above the keyboard (composer bar, message list) glide up; this element
 * is revealed in the freed space, standing in for the keyboard.
 *
 * `onHidden` fires when the element's region is no longer visible — collapsed,
 * or covered by the keyboard that replaced it in a swap — so the caller can keep
 * the content mounted until then instead of it visibly vanishing mid-swap.
 *
 * The element must pin itself to the viewport bottom and must not sit inside a
 * `renderAboveKeyboard` node — see `MediaPanel`.
 */
export const renderBelowKeyboard: Action<
	HTMLElement,
	{ open: boolean; onHidden?: () => void }
> = (node, params) => {
	const surface = registerBelowKeyboard(node, { onHidden: params.onHidden });
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
