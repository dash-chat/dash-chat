const DRAG_THRESHOLD_PX = 8;
const CLOSE_DISTANCE_FRACTION = 0.3;

/** Whether the touch landed on scrolled content, where a drag belongs to the
 * scroller (back up, or back across a carousel) rather than to the sheet. */
function isOnScrolledContent(event: TouchEvent, sheet: HTMLElement) {
	for (const target of event.composedPath()) {
		if (target === sheet) return false;
		if (
			target instanceof HTMLElement &&
			(target.scrollTop > 0 || target.scrollLeft > 0)
		)
			return true;
	}
	return false;
}

/** Lets `sheet` be dragged down with a finger, calling `onClose` when released
 * past a third of its height; a cancelled drag snaps back. Moves the sheet
 * through `transform`, which composes with the `translate` Konsta opens it
 * with. Leaves the offset of a close in place for the caller to clear on the
 * next open. Returns a function that removes the gesture. */
export function swipeToClose(sheet: HTMLElement, onClose: () => void) {
	let startX = 0;
	let startY: number | null = null;
	let dragging = false;
	let offset = 0;

	function snapBack() {
		sheet.style.transition = '';
		sheet.style.transform = '';
	}

	function endDrag() {
		dragging = false;
		startY = null;
	}

	function cancel() {
		if (dragging) {
			snapBack();
			endDrag();
		}
		startY = null;
	}

	function ontouchstart(event: TouchEvent) {
		// A second finger turns the gesture into something else.
		if (dragging || event.touches.length !== 1) {
			cancel();
			return;
		}
		startX = event.touches[0].clientX;
		startY = isOnScrolledContent(event, sheet)
			? null
			: event.touches[0].clientY;
	}

	/** Once the finger has moved far enough: a downward drag takes the sheet,
	 * anything else (a scroll, a horizontal swipe) is left alone. */
	function claimDrag(event: TouchEvent, fromY: number) {
		const dx = Math.abs(event.touches[0].clientX - startX);
		const dy = event.touches[0].clientY - fromY;
		if (Math.max(dx, Math.abs(dy)) < DRAG_THRESHOLD_PX) return;
		// Not cancelable means the browser already claimed the touch for scrolling.
		if (dy <= dx || !event.cancelable) {
			startY = null;
			return;
		}
		dragging = true;
		startY = event.touches[0].clientY;
		sheet.style.transition = 'none';
	}

	function ontouchmove(event: TouchEvent) {
		if (startY === null) return;
		if (!dragging) claimDrag(event, startY);
		if (!dragging) return;
		event.preventDefault();
		offset = Math.max(event.touches[0].clientY - startY, 0);
		sheet.style.transform = `translateY(${offset}px)`;
	}

	function ontouchend() {
		if (!dragging) {
			startY = null;
			return;
		}
		endDrag();
		if (offset > sheet.offsetHeight * CLOSE_DISTANCE_FRACTION) {
			sheet.style.transition = '';
			onClose();
		} else {
			snapBack();
		}
	}

	sheet.addEventListener('touchstart', ontouchstart, { passive: true });
	sheet.addEventListener('touchmove', ontouchmove, { passive: false });
	sheet.addEventListener('touchend', ontouchend);
	sheet.addEventListener('touchcancel', cancel);

	return () => {
		cancel();
		sheet.removeEventListener('touchstart', ontouchstart);
		sheet.removeEventListener('touchmove', ontouchmove);
		sheet.removeEventListener('touchend', ontouchend);
		sheet.removeEventListener('touchcancel', cancel);
	};
}
