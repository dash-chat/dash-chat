// Tracks the soft-keyboard height app-wide. The platform doesn't expose it, so
// we infer it from the webview frame resize the virtual-keyboard-padding plugin
// performs: while the keyboard is up the plugin shrinks `window.innerHeight`, so
// the delta from the tallest height seen is its height. Tracking runs from app
// startup (not just on the chat screen) so the height is already known wherever
// it's first needed, and it's persisted so it survives across sessions too.

const OPEN_THRESHOLD = 120;
const FALLBACK_HEIGHT = 270;
const STORAGE_KEY = 'dashchat:keyboard-height';

let baseline = 0;
let liveHeight = $state(0);
let maxHeight = $state(0);
let tracking = false;

function measure() {
	const h = window.innerHeight;
	if (h > baseline) baseline = h;
	liveHeight = Math.max(0, baseline - h);
	if (liveHeight > maxHeight) {
		maxHeight = liveHeight;
		localStorage.setItem(STORAGE_KEY, String(maxHeight));
	}
}

/**
 * Begin tracking the keyboard height. Idempotent; call once at startup while the
 * keyboard is closed so the baseline height is correct.
 */
export function trackKeyboardHeight() {
	if (tracking || typeof window === 'undefined') return;
	tracking = true;
	baseline = window.innerHeight;
	const stored = Number(localStorage.getItem(STORAGE_KEY));
	if (stored > OPEN_THRESHOLD) maxHeight = stored;
	window.addEventListener('resize', measure);
	window.visualViewport?.addEventListener('resize', measure);
}

export const keyboard = {
	/** Live keyboard height in px, following the open/close animation. */
	get height() {
		return liveHeight;
	},
	get isOpen() {
		return liveHeight > OPEN_THRESHOLD;
	},
	/** The full keyboard height (largest seen, persisted), or a fallback before
	 *  any keyboard has been shown. */
	get reservedHeight() {
		return maxHeight || FALLBACK_HEIGHT;
	},
};
