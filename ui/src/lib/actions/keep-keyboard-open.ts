import type { Action } from 'svelte/action';

// Keep the mobile keyboard open by preventing taps anywhere in the composer
// (buttons, chrome, the attachment panel) from moving focus off the textarea.
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
