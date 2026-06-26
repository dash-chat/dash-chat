import type { Action } from 'svelte/action';

/**
 * Invoke `onEnter` once the element comes within `rootMargin` of the viewport,
 * then stop observing. Replaces native `loading="lazy"` when "load" means
 * something other than setting an `<img src>` (here: fetch + cache blob bytes),
 * so deferral until near-viewport is preserved without an eager fetch.
 */
export const inView: Action<
	HTMLElement,
	{ onEnter: () => void; rootMargin?: string } | undefined
> = (node, params) => {
	if (!params) return;
	let onEnter = params.onEnter;
	const observer = new IntersectionObserver(
		entries => {
			if (entries.some(entry => entry.isIntersecting)) {
				observer.disconnect();
				onEnter();
			}
		},
		{ rootMargin: params.rootMargin ?? '300px' },
	);
	observer.observe(node);
	return {
		update(next) {
			if (next) onEnter = next.onEnter;
		},
		destroy() {
			observer.disconnect();
		},
	};
};
