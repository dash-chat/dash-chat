import type { Action } from 'svelte/action';

interface LongPressParams {
	onLongPress: (e: MouseEvent | TouchEvent, element: HTMLElement) => void;
	duration?: number;
}

/** Stops iOS from answering the press with its own selection and link UI,
 * and Android from starting a link drag with its title/URL preview box. */
function suppressNativeLongPress(target: HTMLElement) {
	target.style.setProperty('-webkit-user-select', 'none');
	target.style.setProperty('user-select', 'none');
	target.style.setProperty('-webkit-touch-callout', 'none');
	target.draggable = false;
}

export function longPressHandlers({ onLongPress, duration }: LongPressParams) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	let triggered = false;

	return {
		ontouchstart(e: TouchEvent) {
			// `currentTarget` is nulled once dispatch ends, so it must be read before the timer.
			const element = e.currentTarget;
			if (!(element instanceof HTMLElement)) return;
			suppressNativeLongPress(element);
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
		},
		ontouchend(e: TouchEvent) {
			clearTimeout(timer);
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
