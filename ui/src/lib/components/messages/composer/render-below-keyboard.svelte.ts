import { isIos } from '$lib/utils/environment';
import { keyboard, pulseKeyboardTracking } from '$lib/utils/keyboard.svelte';
import type { Action } from 'svelte/action';

type Phase = 'closed' | 'opening' | 'shown' | 'yielding';

const YIELD_BACKSTOP_MS = 500;

/**
 * Makes the attached node sit in the keyboard's slot: it takes over the node's
 * height and drives it to track the live keyboard height, so a sibling input bar
 * stays pinned while the panel and the keyboard swap places.
 *
 *   - `open` true with the keyboard up → blur it; as it retracts the height grows
 *     by exactly the vacated space (no animation).
 *   - input refocused while shown → the keyboard reclaims the slot; the height
 *     shrinks to match (on iOS the content is hidden right away, since
 *     visualViewport lags the keyboard, so it never shows over the rising
 *     keyboard), then `onClose` fires.
 *   - open/close with no keyboard → the panel just appears/disappears.
 *
 * The host owns nothing but `open` (and mounts the panel content on it); the
 * action calls `onClose` once the keyboard has taken the slot back.
 */
export const renderBelowKeyboard: Action<
	HTMLElement,
	{ open: boolean; onClose: () => void }
> = (node, params) => {
	let open = $state(params.open);
	let onClose = params.onClose;
	let phase: Phase = 'closed';
	let wasOpen = false;
	let timer: ReturnType<typeof setTimeout> | undefined;

	// Show/hide the slot's content while keeping the slot (and its background)
	// in place. Toggled directly here, alongside the height, so the host node
	// needs no styling of its own.
	function setContentHidden(hidden: boolean) {
		for (const child of Array.from(node.children) as HTMLElement[]) {
			child.style.visibility = hidden ? 'hidden' : '';
		}
	}

	// Start handing the slot back to the keyboard: keep the slot (so a sibling
	// input bar stays pinned) but, on iOS, hide the content so it never renders
	// on top of the rising keyboard during the visualViewport lag.
	function enterYield() {
		phase = 'yielding';
		if (isIos) setContentHidden(true);
		// The keyboard is about to rise; poll the layout each frame so the slot
		// collapses in step with it rather than lagging a (late) resize event.
		pulseKeyboardTracking();
		clearTimeout(timer);
		// Backstop in case the keyboard settles shorter than the reserved height,
		// so `slot` never quite reaches 0.
		timer = setTimeout(() => onClose(), YIELD_BACKSTOP_MS);
	}

	// On iOS the input regaining focus is the earliest signal the keyboard is
	// about to reclaim the slot — visualViewport lags it by ~130ms — so yield as
	// soon as it's focused rather than waiting for the (late) height change.
	function onFocusIn(event: FocusEvent) {
		const target = event.target;
		if (
			isIos &&
			open &&
			phase === 'shown' &&
			target instanceof HTMLElement &&
			(target.tagName === 'INPUT' ||
				target.tagName === 'TEXTAREA' ||
				target.isContentEditable)
		) {
			enterYield();
		}
	}
	document.addEventListener('focusin', onFocusIn, true);

	const stop = $effect.root(() => {
		$effect(() => {
			const isOpen = open;
			const kbOpen = keyboard.isOpen;
			// The slot the panel should fill: what the keyboard isn't currently using.
			const slot = Math.max(0, keyboard.reservedHeight - keyboard.height);

			if (isOpen && !wasOpen) {
				// Just opened: if the keyboard is up, dismiss it and grow into the
				// space it vacates; otherwise show at full height immediately.
				phase = kbOpen ? 'opening' : 'shown';
				setContentHidden(false);
				if (kbOpen) {
					(document.activeElement as HTMLElement | null)?.blur();
					pulseKeyboardTracking();
				}
			} else if (!isOpen) {
				phase = 'closed';
				setContentHidden(false);
				clearTimeout(timer);
			}
			wasOpen = isOpen;

			if (phase === 'opening' && !kbOpen) {
				phase = 'shown';
			} else if (phase === 'shown' && kbOpen) {
				// The keyboard rose without `onFocusIn` firing first — hand the slot back.
				enterYield();
			} else if (phase === 'yielding' && slot <= 1) {
				clearTimeout(timer);
				phase = 'closed';
				onClose();
			}

			node.style.height = `${phase === 'closed' ? 0 : slot}px`;
		});
	});

	return {
		update(next) {
			open = next.open;
			onClose = next.onClose;
		},
		destroy() {
			clearTimeout(timer);
			document.removeEventListener('focusin', onFocusIn, true);
			stop();
		},
	};
};
