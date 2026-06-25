import { keyboard } from '$lib/utils/keyboard.svelte';
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
 *   - input refocused while shown → the rising keyboard reclaims the slot, the
 *     height shrinks to match, then `onClose` fires.
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
				if (kbOpen) (document.activeElement as HTMLElement | null)?.blur();
			} else if (!isOpen) {
				phase = 'closed';
				clearTimeout(timer);
			}
			wasOpen = isOpen;

			if (phase === 'opening' && !kbOpen) {
				phase = 'shown';
			} else if (phase === 'shown' && kbOpen) {
				// The keyboard came back up (input refocused) — hand the slot back.
				phase = 'yielding';
				clearTimeout(timer);
				// Backstop in case the keyboard settles shorter than the reserved
				// height, so `slot` never quite reaches 0.
				timer = setTimeout(() => onClose(), YIELD_BACKSTOP_MS);
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
			stop();
		},
	};
};
