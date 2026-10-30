import type { Action } from 'svelte/action';

interface LongPressParams {
	onLongPress: (e: MouseEvent | TouchEvent, element: HTMLElement) => void;
	/** Called once a touch has rested on the element for a moment, well before
	 * the long-press fires. */
	onPressStart?: () => void;
	duration?: number;
}

/** Stops iOS from answering the press with its own selection and link UI. */
function suppressNativeLongPress(target: HTMLElement) {
	target.style.setProperty('-webkit-user-select', 'none');
	target.style.setProperty('user-select', 'none');
	target.style.setProperty('-webkit-touch-callout', 'none');
}

// A flick starts with a touch too; only a finger that stays put is pressing.
const PRESS_SETTLE_MS = 150;

export function longPressHandlers({
	onLongPress,
	onPressStart,
	duration,
}: LongPressParams) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	let settleTimer: ReturnType<typeof setTimeout> | undefined;
	let triggered = false;

	return {
		ontouchstart(e: TouchEvent) {
			// `currentTarget` is nulled once dispatch ends, so it must be read before the timer.
			const element = e.currentTarget;
			if (!(element instanceof HTMLElement)) return;
			suppressNativeLongPress(element);
			clearTimeout(settleTimer);
			if (onPressStart) settleTimer = setTimeout(onPressStart, PRESS_SETTLE_MS);
			clearTimeout(timer);
			triggered = false;
			timer = setTimeout(() => {
				triggered = true;
				window.getSelection()?.removeAllRanges();
				if (element.isConnected) onLongPress(e, element);
			}, duration ?? 500);
		},
		ontouchmove() {
			clearTimeout(timer);
			clearTimeout(settleTimer);
		},
		ontouchend(e: TouchEvent) {
			clearTimeout(timer);
			clearTimeout(settleTimer);
			if (triggered) {
				triggered = false;
				e.preventDefault();
			}
		},
		oncontextmenu(e: MouseEvent) {
			if (!(e.currentTarget instanceof HTMLElement)) return;
			e.preventDefault();
			onLongPress(e, e.currentTarget);
		},
	};
}

export const longpress: Action<HTMLElement, LongPressParams> = (
	node,
	params,
) => {
	suppressNativeLongPress(node);

	let handlers = longPressHandlers(params);

	function attach() {
		node.addEventListener('touchstart', handlers.ontouchstart, {
			passive: true,
		});
		node.addEventListener('touchmove', handlers.ontouchmove, { passive: true });
		node.addEventListener('touchend', handlers.ontouchend);
		node.addEventListener('contextmenu', handlers.oncontextmenu);
	}

	function detach() {
		node.removeEventListener('touchstart', handlers.ontouchstart);
		node.removeEventListener('touchmove', handlers.ontouchmove);
		node.removeEventListener('touchend', handlers.ontouchend);
		node.removeEventListener('contextmenu', handlers.oncontextmenu);
	}

	attach();

	return {
		update(newParams: LongPressParams) {
			detach();
			handlers = longPressHandlers(newParams);
			attach();
		},
		destroy() {
			detach();
		},
	};
};
