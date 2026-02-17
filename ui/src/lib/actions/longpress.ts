import type { Action } from 'svelte/action';

interface LongPressParams {
	onLongPress: (e: MouseEvent | TouchEvent) => void;
	duration?: number;
}

export const longpress: Action<HTMLElement, LongPressParams> = (
	node,
	params,
) => {
	let timer: ReturnType<typeof setTimeout> | undefined;
	let triggered = false;

	function onTouchStart(e: TouchEvent) {
		triggered = false;
		timer = setTimeout(() => {
			triggered = true;
			window.getSelection()?.removeAllRanges();
			params.onLongPress(e);
		}, params.duration ?? 500);
	}

	function onTouchMove() {
		clearTimeout(timer);
	}

	function onTouchEnd(e: TouchEvent) {
		clearTimeout(timer);
		if (triggered) {
			e.preventDefault();
		}
	}

	function onContextMenu(e: MouseEvent) {
		e.preventDefault();
		params.onLongPress(e);
	}

	node.addEventListener('touchstart', onTouchStart, { passive: true });
	node.addEventListener('touchmove', onTouchMove, { passive: true });
	node.addEventListener('touchend', onTouchEnd);
	node.addEventListener('contextmenu', onContextMenu);

	return {
		update(newParams: LongPressParams) {
			params = newParams;
		},
		destroy() {
			clearTimeout(timer);
			node.removeEventListener('touchstart', onTouchStart);
			node.removeEventListener('touchmove', onTouchMove);
			node.removeEventListener('touchend', onTouchEnd);
			node.removeEventListener('contextmenu', onContextMenu);
		},
	};
};
