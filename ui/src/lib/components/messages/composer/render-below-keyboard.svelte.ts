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

	// Pin the slot's height to the viewport via CSS (Android): JS resize handling
	// lags the frame resize by a paint, which flashed the input bar out of place
	// mid-swap. The wanted height is always `reserved - keyboardHeight`, which is
	// linear in the viewport height, so calc(100dvh - Cpx) tracks it atomically
	// in the same relayout as any frame resize. C is stable for the panel's whole
	// lifetime; it's computed from live metrics whenever a phase starts.
	function pinSlot() {
		// Fractional viewport height: clientHeight rounds to an integer while
		// 100dvh is fractional on non-integer DPRs, and the sub-pixel error shows
		// up as a 1px step in the input bar between the panel and keyboard states.
		const viewportNow =
			window.visualViewport?.height ?? document.documentElement.clientHeight;
		const c = viewportNow + keyboard.height - keyboard.reservedHeight;
		node.style.height = `max(0px, calc(100dvh - ${c}px))`;
	}

	// Start handing the slot back to the keyboard: keep the slot (so a sibling
	// input bar stays pinned) but, on iOS, hide the content so it never renders
	// on top of the rising keyboard during the visualViewport lag.
	function enterYield() {
		phase = 'yielding';
		if (isIos) setContentHidden(true);
		else pinSlot();
		// The keyboard is about to rise; poll the layout each frame so the slot
		// collapses in step with it rather than lagging a (late) resize event.
		pulseKeyboardTracking();
		clearTimeout(timer);
		// Backstop in case the keyboard settles shorter than the reserved height
		// (so `slot` never quite reaches 0) or never rises at all (hardware
		// keyboard).
		timer = setTimeout(() => onClose(), YIELD_BACKSTOP_MS);
	}

	// The input regaining focus is the earliest signal the keyboard is about to
	// reclaim the slot — the viewport metrics lag it (~130ms on iOS, a paint or
	// two on Android) — so yield as soon as it's focused rather than waiting for
	// the (late) height change. 'opening' counts too: the panel is visible while
	// the keyboard it displaced is still retracting, and a quick close-tap there
	// must not fall back to the laggy unpinned path.
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
				if (!isIos) pinSlot();
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
			} else if (
				phase === 'yielding' &&
				node.getBoundingClientRect().height <= 1
			) {
				// Close on the *rendered* height, not the tracked keyboard height:
				// visualViewport (which feeds keyboard.height) leads the layout
				// viewport by a frame, and closing early swaps the pinned slot for
				// 0px while the layout is still tall — dropping the bar for a frame.
				clearTimeout(timer);
				phase = 'closed';
				onClose();
			}

			// While CSS-pinned (Android, any live phase) the height must not be
			// overwritten per-frame, or the JS lag the pin avoids would come back.
			const cssPinned = phase !== 'closed' && !isIos;
			if (!cssPinned) node.style.height = `${phase === 'closed' ? 0 : slot}px`;
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
