import type { Action } from 'svelte/action';

const OVERLAY_SCROLLBAR_WIDTH = 12;

/**
 * Sets the element's `right` to the width of the scroll container's
 * scrollbar so the scrollbar handle stays uncovered. Apply to absolutely
 * positioned overlays that sit above a scroll area.
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
		node.style.right = `${compute()}px`;
	};

	const attach = (el: HTMLElement | null) => {
		ro?.disconnect();
		mo?.disconnect();
		current = el;
		if (!el) {
			node.style.right = '0px';
			return;
		}
		update();
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
			node.style.right = '';
		},
	};
};
