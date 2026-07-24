import type { Action } from 'svelte/action';
import { registerAboveKeyboard } from 'tauri-plugin-virtual-keyboard';

/**
 * Keep a node rendered above the keyboard: across a keyboard or below-keyboard
 * surface transition it glides to its new spot on the compositor, instead of
 * being re-laid-out every frame (which is what makes the message list tremble).
 *
 * `onPreviewLayoutMeasure` runs while the show-glide's final layout is applied
 * invisibly for measurement — see `registerAboveKeyboard`. Captured once at
 * mount; pass a stable function.
 */
export const renderAboveKeyboard: Action<
	HTMLElement,
	{ onPreviewLayoutMeasure?: () => void } | undefined
> = (node, params) => ({
	destroy: registerAboveKeyboard(node, params),
});
