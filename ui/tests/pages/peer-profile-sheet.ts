import { S } from '../selectors';

export const selectors = S.peerProfileSheet;

/** True if the PeerProfileSheet is currently open and visible. */
export function isPeerProfileSheetOpen(): boolean {
	const inner = document.querySelector(selectors.root);
	if (!inner) return false;
	const root = inner.closest('.k-sheet, .k-dialog');
	if (!root) return false;
	if (root.classList.contains('k-sheet')) {
		return root.classList.contains('-translate-y-full');
	}
	return !root.classList.contains('opacity-0');
}
