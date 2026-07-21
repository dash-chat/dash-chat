import type { Action } from 'svelte/action';
import {
	hideKeyboard,
	keyboard,
	onKeyboardWillShow,
} from 'tauri-plugin-virtual-keyboard';

const YIELD_BACKSTOP_MS = 500;

/**
 * Makes the attached node sit in the keyboard's slot: it takes over the node's
 * height and drives it to fill whatever part of the keyboard's reserved space
 * the keyboard isn't covering, so a sibling input bar stays pinned while the
 * panel and the keyboard swap places.
 *
 *   - `open` true with the keyboard up → hide it; as it retracts the height
 *     grows by exactly the vacated space.
 *   - input refocused while shown → the keyboard reclaims the slot; the height
 *     shrinks to match, then `onClose` fires.
 *   - open/close with no keyboard → the panel just appears/disappears.
 *
 * The host owns nothing but `open` (and mounts the panel content on it); the
 * action calls `onClose` once the keyboard has taken the slot back.
 */
export const renderBelowKeyboard: Action<
	HTMLElement,
	{ open: boolean; onClose: () => void }
> = (node, params) => {
	let open = params.open;
	let onClose = params.onClose;
	let wasOpen = false;
	let held = false;
	let timer: ReturnType<typeof setTimeout> | undefined;

	// The plugin maintains --keyboard-height per frame natively, so this CSS
	// tracks the swap atomically with the layout's own keyboard padding — no
	// JS-side frame chasing.
	function fillSlot() {
		node.style.height = `max(0px, calc(${keyboard.reservedHeight.value}px - var(--keyboard-height, 0px)))`;
	}

	function closeNow() {
		clearTimeout(timer);
		held = false;
		node.style.height = '0px';
		onClose();
	}

	function editableFocused() {
		const el = document.activeElement;
		return (
			el instanceof HTMLElement &&
			(el.tagName === 'INPUT' ||
				el.tagName === 'TEXTAREA' ||
				el.isContentEditable)
		);
	}

	// While the slot is held, a rising keyboard is reclaiming it. The per-frame
	// swap is the CSS formula's job; JS only schedules the endpoint: collapse
	// and report the close once the animation has landed.
	const unsubscribe = onKeyboardWillShow(({ durationMs }) => {
		if (!held) return;
		clearTimeout(timer);
		timer = setTimeout(closeNow, durationMs + 50);
	});

	function evaluate() {
		if (open && !wasOpen) {
			// Just opened: if the keyboard is up, dismiss it and grow into the
			// space it vacates; otherwise show at full height immediately.
			// A pending yield must not outlive the reopen.
			clearTimeout(timer);
			held = true;
			fillSlot();
			if (keyboard.isOpen.value) {
				(document.activeElement as HTMLElement | null)?.blur();
				hideKeyboard();
			}
		} else if (!open && wasOpen && held) {
			if (editableFocused()) {
				// The host cleared `open` while handing focus to an input (so the
				// button state flips instantly): keep the slot until the rising
				// keyboard claims it, so the input bar stays pinned during the
				// swap. Backstop in case no keyboard rises (hardware keyboard).
				clearTimeout(timer);
				timer = setTimeout(closeNow, YIELD_BACKSTOP_MS);
			} else {
				closeNow();
			}
		} else if (!open && !held) {
			node.style.height = '0px';
		}
		wasOpen = open;
	}

	evaluate();

	return {
		update(next) {
			open = next.open;
			onClose = next.onClose;
			evaluate();
		},
		destroy() {
			clearTimeout(timer);
			unsubscribe();
		},
	};
};
