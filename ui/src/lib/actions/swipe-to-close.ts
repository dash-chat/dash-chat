import type { Action } from 'svelte/action';

const DRAG_THRESHOLD_PX = 8;
const CLOSE_DISTANCE_FRACTION = 0.3;

/** Whether the touch landed on content that is scrolled down, where a
 * downward drag must scroll it back up rather than move the sheet. */
function isOnScrolledContent(event: TouchEvent, sheet: HTMLElement) {
	for (const target of event.composedPath()) {
		if (target === sheet) return false;
		if (target instanceof HTMLElement && target.scrollTop > 0) return true;
	}
	return false;
}

/** Drop the drag offset once the sheet has slid out, so it reopens in place. */
function clearOffsetAfterClose(sheet: HTMLElement) {
	sheet.addEventListener(
		'transitionend',
		() => {
			sheet.style.transition = 'none';
			sheet.style.transform = '';
			// Commit the reset before restoring the class transition.
			void sheet.offsetHeight;
			sheet.style.transition = '';
		},
		{ once: true },
	);
}

/** Lets the Konsta sheet around `node` be dragged down with a finger, calling
 * `onClose` when released past a third of its height. Listens on the sheet
 * itself, since its own padding sits outside `node`. Moves the sheet through
 * `transform`, which composes with the `translate` Konsta opens it with. */
export const swipeToClose: Action<HTMLElement, () => void> = (
	node,
	onClose,
) => {
	const enclosingSheet = node.closest<HTMLElement>('.k-sheet');
	if (!enclosingSheet) return;
	const sheet: HTMLElement = enclosingSheet;
	let close = onClose;
	let startY: number | null = null;
	let offset = 0;

	function ontouchstart(event: TouchEvent) {
		const canDrag =
			event.touches.length === 1 && !isOnScrolledContent(event, sheet);
		startY = canDrag ? event.touches[0].clientY : null;
		offset = 0;
	}

	function ontouchmove(event: TouchEvent) {
		if (startY === null) return;
		const dy = event.touches[0].clientY - startY;
		if (offset === 0 && dy < DRAG_THRESHOLD_PX) return;
		// Not cancelable means the browser already claimed the touch for scrolling.
		if (!event.cancelable) {
			startY = null;
			return;
		}
		event.preventDefault();
		offset = Math.max(dy, 1);
		sheet.style.transition = 'none';
		sheet.style.transform = `translateY(${offset}px)`;
	}

	function ontouchend() {
		startY = null;
		if (offset === 0) return;
		sheet.style.transition = '';
		if (offset > sheet.offsetHeight * CLOSE_DISTANCE_FRACTION) {
			clearOffsetAfterClose(sheet);
			close();
		} else {
			sheet.style.transform = '';
		}
		offset = 0;
	}

	sheet.addEventListener('touchstart', ontouchstart, { passive: true });
	sheet.addEventListener('touchmove', ontouchmove, { passive: false });
	sheet.addEventListener('touchend', ontouchend);
	sheet.addEventListener('touchcancel', ontouchend);

	return {
		update(newOnClose: () => void) {
			close = newOnClose;
		},
		destroy() {
			sheet.removeEventListener('touchstart', ontouchstart);
			sheet.removeEventListener('touchmove', ontouchmove);
			sheet.removeEventListener('touchend', ontouchend);
			sheet.removeEventListener('touchcancel', ontouchend);
		},
	};
};
