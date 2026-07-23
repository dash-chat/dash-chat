import type { Action } from 'svelte/action';
import { registerAboveKeyboard } from 'tauri-plugin-virtual-keyboard';

/**
 * Keep a node rendered above the keyboard: across a keyboard or below-keyboard
 * surface transition it glides to its new spot on the compositor, instead of
 * being re-laid-out every frame (which is what makes the message list tremble).
 *
 * The bottom-most such node also wants the `pb-grounded-safe` class, which adds
 * home-indicator padding only while nothing occupies the slot below it.
 */
export const renderAboveKeyboard: Action = node => ({
	destroy: registerAboveKeyboard(node),
});
