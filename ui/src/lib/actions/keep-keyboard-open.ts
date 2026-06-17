import type { Action } from 'svelte/action';

const INTERACTIVE =
	'button, a, input, textarea, label, select, [role="button"]';

/** Keep the mobile keyboard open by preventing taps on the composer's
 * non-interactive chrome from stealing focus from the textarea. Taps on
 * interactive controls (buttons, the textarea, …) are left untouched — on
 * touch, calling preventDefault on their pointer-down would also cancel the
 * synthesized click. */
export const keepKeyboardOpen: Action<HTMLElement> = node => {
	function handle(event: Event) {
		const target = event.target;
		if (target instanceof Element && target.closest(INTERACTIVE)) return;
		event.preventDefault();
	}
	const options = { passive: false };
	node.addEventListener('mousedown', handle, options);
	node.addEventListener('touchstart', handle, options);
	node.addEventListener('pointerdown', handle, options);
	return {
		destroy() {
			node.removeEventListener('mousedown', handle);
			node.removeEventListener('touchstart', handle);
			node.removeEventListener('pointerdown', handle);
		},
	};
};
