import { useSignal } from '$lib/stores/use-signal';
import type { Action } from 'svelte/action';
import { hideKeyboard, keyboard } from 'tauri-plugin-virtual-keyboard';

const keyboardIsOpen = useSignal(() => keyboard.isOpen.value);

type Phase = 'closed' | 'opening' | 'shown' | 'yielding';

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
	let phase: Phase = 'closed';
	let wasOpen = false;
	let kbOpen = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let raf: number | undefined;

	// The plugin maintains --keyboard-height per frame natively, so this CSS
	// tracks the swap atomically with the layout's own keyboard padding — no
	// JS-side frame chasing.
	function fillSlot() {
		node.style.height = `max(0px, calc(${keyboard.reservedHeight.value}px - var(--keyboard-height, 0px)))`;
	}

	function stopRaf() {
		if (raf !== undefined) cancelAnimationFrame(raf);
		raf = undefined;
	}

	// Finish a yield: collapse the slot and report the close. Done directly (not
	// via the host flipping `open`) because the host may have cleared `open`
	// already, in which case no update would arrive.
	function closeNow() {
		clearTimeout(timer);
		stopRaf();
		phase = 'closed';
		node.style.height = '0px';
		onClose();
	}

	// Start handing the slot back to the keyboard: keep the slot (so a sibling
	// input bar stays pinned) until the rising keyboard has visually reclaimed
	// it. The rendered height follows --keyboard-height, not reactive state, so
	// it's polled per frame.
	function enterYield() {
		phase = 'yielding';
		fillSlot();
		clearTimeout(timer);
		// Backstop in case the keyboard settles shorter than the reserved height
		// (so the slot never quite reaches 0) or never rises at all (hardware
		// keyboard).
		timer = setTimeout(closeNow, YIELD_BACKSTOP_MS);
		stopRaf();
		const tick = () => {
			if (phase !== 'yielding') return;
			if (node.getBoundingClientRect().height <= 1) closeNow();
			else raf = requestAnimationFrame(tick);
		};
		raf = requestAnimationFrame(tick);
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

	// The input regaining focus is the earliest signal the keyboard is about to
	// reclaim the slot — even the native will-show event trails it — so yield as
	// soon as it's focused. 'opening' counts too: the panel is visible while the
	// keyboard it displaced is still retracting, and a quick close-tap there
	// must yield rather than leave the slot behind.
	function onFocusIn(event: FocusEvent) {
		const target = event.target;
		if (
			open &&
			(phase === 'shown' || phase === 'opening') &&
			target instanceof HTMLElement &&
			(target.tagName === 'INPUT' ||
				target.tagName === 'TEXTAREA' ||
				target.isContentEditable)
		) {
			enterYield();
		}
	}
	document.addEventListener('focusin', onFocusIn, true);

	function evaluate() {
		if (open && !wasOpen) {
			// Just opened: if the keyboard is up, dismiss it and grow into the
			// space it vacates; otherwise show at full height immediately.
			// A pending yield backstop must not outlive the reopen.
			clearTimeout(timer);
			stopRaf();
			phase = kbOpen ? 'opening' : 'shown';
			fillSlot();
			if (kbOpen) {
				(document.activeElement as HTMLElement | null)?.blur();
				hideKeyboard();
			}
		} else if (!open) {
			if (
				wasOpen &&
				(phase === 'shown' || phase === 'opening') &&
				editableFocused()
			) {
				// The host cleared `open` while handing focus to an input (so the
				// button state flips instantly): keep the slot until the keyboard
				// claims it, so the input bar stays pinned during the swap.
				enterYield();
			} else if (phase !== 'yielding') {
				phase = 'closed';
				clearTimeout(timer);
				stopRaf();
				node.style.height = '0px';
			}
		}
		wasOpen = open;

		if (phase === 'opening' && !kbOpen) {
			phase = 'shown';
		} else if (phase === 'shown' && kbOpen) {
			// The keyboard rose without `onFocusIn` firing first — hand the slot back.
			enterYield();
		}
	}

	const unsubscribe = keyboardIsOpen.subscribe(value => {
		kbOpen = value;
		evaluate();
	});

	return {
		update(next) {
			open = next.open;
			onClose = next.onClose;
			evaluate();
		},
		destroy() {
			clearTimeout(timer);
			stopRaf();
			document.removeEventListener('focusin', onFocusIn, true);
			unsubscribe();
		},
	};
};
