import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';

export class PeerProfileSheet extends TestHelper {
	root = this.el(tid('peer-profile-sheet'));

	/** True if the sheet is open and visible (not the slide-out dismissed state). */
	isOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const inner = document.querySelector(sel);
			if (!inner) return false;
			const root = inner.closest('.k-sheet, .k-dialog');
			if (!root) return false;
			if (root.classList.contains('k-sheet')) {
				return root.classList.contains('-translate-y-full');
			}
			return !root.classList.contains('opacity-0');
		}, tid('peer-profile-sheet'));
	}
}
