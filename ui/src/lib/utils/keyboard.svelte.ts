// Tracks the soft-keyboard height app-wide. The platform doesn't expose it, so
// we infer it from the webview frame resize the virtual-keyboard-padding plugin
// performs: while the keyboard is up the plugin shrinks `window.innerHeight`, so
// the delta from the tallest height seen is its height. Tracking runs from app
// startup (not just on the chat screen) so the height is already known wherever
// it's first needed, and it's persisted so it survives across sessions too.

const OPEN_THRESHOLD = 120;
const FALLBACK_HEIGHT = 270;
// A keyboard never exceeds this fraction of the viewport; a larger shrink is some
// other layout change, not the keyboard, so it must not poison the tracking.
const MAX_KEYBOARD_FRACTION = 0.6;
const STORAGE_KEY = 'dashchat:keyboard-height';

let baseline = 0;
let baselineWidth = 0;
let liveHeight = $state(0);
let maxHeight = $state(0);
let tracking = false;

// Prefer visualViewport: on iOS WKWebView it's the reliable signal for keyboard
// geometry and doesn't always move in lockstep with innerHeight.
function viewport() {
	const vv = window.visualViewport;
	return {
		height: vv?.height ?? window.innerHeight,
		width: vv?.width ?? window.innerWidth,
	};
}

function measure() {
	const { height: h, width: w } = viewport();
	// The keyboard never changes the viewport width; a width change is a rotation
	// or layout change, so re-baseline rather than read the shorter side as a
	// permanently-open keyboard. `baseline` only grows within one orientation.
	if (w !== baselineWidth) {
		baselineWidth = w;
		baseline = h;
	} else if (h > baseline) {
		baseline = h;
	}
	liveHeight = Math.max(0, baseline - h);
	// Only adopt a plausibly keyboard-sized shrink, so a spurious frame can't
	// corrupt the persisted height (which otherwise only ever grows).
	if (liveHeight > maxHeight && liveHeight < baseline * MAX_KEYBOARD_FRACTION) {
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
	const { height, width } = viewport();
	baseline = height;
	baselineWidth = width;
	const stored = Number(localStorage.getItem(STORAGE_KEY));
	if (stored > OPEN_THRESHOLD && stored < baseline * MAX_KEYBOARD_FRACTION) {
		maxHeight = stored;
	}
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
