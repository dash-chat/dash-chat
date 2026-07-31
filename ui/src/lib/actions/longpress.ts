import type { Action } from 'svelte/action';

interface LongPressParams {
	onLongPress: (e: MouseEvent | TouchEvent) => void;
	duration?: number;
}

let timer: ReturnType<typeof setTimeout> | undefined;
let triggered = false;

/** Stops iOS from answering the press with its own selection and link UI. */
function suppressNativeLongPress(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return;
	target.style.setProperty('-webkit-user-select', 'none');
	target.style.setProperty('user-select', 'none');
	target.style.setProperty('-webkit-touch-callout', 'none');
}

export function longPressHandlers({ onLongPress, duration }: LongPressParams) {
	return {
		ontouchstart(e: TouchEvent) {
			suppressNativeLongPress(e.currentTarget);
			triggered = false;
			timer = setTimeout(() => {
				triggered = true;
				window.getSelection()?.removeAllRanges();
				if ((e.target as Element).isConnected) onLongPress(e);
			}, duration ?? 500);
		},
		ontouchmove() {
			clearTimeout(timer);
		},
		ontouchend(e: TouchEvent) {
			clearTimeout(timer);
			if (triggered) e.preventDefault();
		},
		oncontextmenu(e: MouseEvent) {
			e.preventDefault();
			onLongPress(e);
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
			clearTimeout(timer);
			detach();
		},
	};
};
