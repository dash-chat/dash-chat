import type { Action } from 'svelte/action';
import { keepKeyboardOpen as keepOpen } from 'tauri-plugin-virtual-keyboard';

/** Keep taps on the node's non-editable children (buttons, chrome, panels)
 * from moving focus off the focused input, so the keyboard stays up. */
export const keepKeyboardOpen: Action<HTMLElement> = node => ({
	destroy: keepOpen(node),
});
