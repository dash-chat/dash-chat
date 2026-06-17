import type { Action } from 'svelte/action';

/** Keep the mobile keyboard open by preventing taps anywhere in the composer
 * (buttons, chrome, the attachment panel) from moving focus off the textarea.
 *
 * Only `mousedown` is intercepted: preventing its default stops the focus
 * change while still letting the synthesized `click` fire, so buttons keep
 * working — including on touch. Preventing `touchstart`/`pointerdown` instead
 * would also cancel the tap's click, which is why they are deliberately left
 * alone. Taps on the textarea itself are passed through so it can focus. */
export const keepKeyboardOpen: Action<HTMLElement> = node => {
	function handle(event: Event) {
		if (!(event.target instanceof HTMLTextAreaElement)) {
			event.preventDefault();
		}
	}
	node.addEventListener('mousedown', handle);
	return {
		destroy() {
			node.removeEventListener('mousedown', handle);
		},
	};
};
