import type { Action } from 'svelte/action';

const OVERLAY_SCROLLBAR_WIDTH = 12;

/**
 * Insets the element by the scroll container's scrollbar width on the
 * inline-end side so its content (and click target) stays clear of the
 * scrollbar handle. The `[data-scrollbar-inset]::after` rule in app.css
 * draws a `pointer-events: none` filler over the gutter so the bar's
 * background appears continuous while the scrollbar stays clickable.
 *
 * Apply to absolutely positioned overlays (e.g. message input bars) that
 * sit above a scroll area. Logical properties so it works in both LTR
 * (gutter on the right) and RTL (gutter on the left).
 *
 * Handles two scrollbar modes:
 *   - Reserved gutter (Windows/Linux/macOS "Always show"): uses the
 *     actual gutter width from offsetWidth - clientWidth.
 *   - Overlay (macOS default): no reserved width, but the bar fades in
 *     over content during scroll. Reserves a fixed width when the
 *     content overflows AND a fine pointer is present.
 */
export const scrollbarInset: Action<HTMLElement, HTMLElement | null> = (
	node,
	scrollContainer,
) => {
	node.dataset.scrollbarInset = '';

	let ro: ResizeObserver | null = null;
	let mo: MutationObserver | null = null;
	let current: HTMLElement | null = null;
	const finePointer = matchMedia('(pointer: fine)').matches;

	const compute = (): number => {
		if (!current) return 0;
		const reserved = current.offsetWidth - current.clientWidth;
		if (reserved > 0) return reserved;
		if (finePointer && current.scrollHeight > current.clientHeight) {
			return OVERLAY_SCROLLBAR_WIDTH;
		}
		return 0;
	};

	const update = () => {
		const w = compute();
		node.style.insetInlineEnd = `${w}px`;
		node.style.setProperty('--scrollbar-inset-width', `${w}px`);
	};

	const attach = (el: HTMLElement | null) => {
		ro?.disconnect();
		mo?.disconnect();
		current = el;
		update();
		if (!el) return;
		ro = new ResizeObserver(update);
		ro.observe(el);
		mo = new MutationObserver(update);
		mo.observe(el, { childList: true, subtree: true });
	};

	attach(scrollContainer);

	return {
		update(next) {
			attach(next);
		},
		destroy() {
			ro?.disconnect();
			mo?.disconnect();
			delete node.dataset.scrollbarInset;
			node.style.insetInlineEnd = '';
			node.style.removeProperty('--scrollbar-inset-width');
		},
	};
};
