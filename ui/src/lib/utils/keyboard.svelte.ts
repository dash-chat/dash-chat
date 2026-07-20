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

// Track whichever viewport metric has shrunk the most. As the iOS keyboard opens
// the layout viewport (`documentElement.clientHeight`) leads both `innerHeight`
// and `visualViewport` by a frame or two; since the panel slot is laid out
// against the layout viewport, following the leading signal keeps the slot and
// the layout in lockstep and avoids a transient where the slot is still full
// while the layout has already shrunk.
function viewport() {
	const vv = window.visualViewport;
	const layout = document.documentElement.clientHeight || window.innerHeight;
	return {
		height: Math.min(
			vv?.height ?? window.innerHeight,
			window.innerHeight,
			layout,
		),
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

let pulseRaf: number | undefined;
let pulseUntil = 0;

/**
 * Re-measure every frame for a short window. The iOS layout viewport can shrink
 * a frame or two before any `resize`/`visualViewport` event fires (or without
 * firing one at all), so event-driven measuring alone leaves the height stale
 * mid-animation. Call when a keyboard transition is about to start.
 */
export function pulseKeyboardTracking(durationMs = 400) {
	if (!tracking) return;
	pulseUntil = Math.max(pulseUntil, performance.now() + durationMs);
	if (pulseRaf !== undefined) return;
	const tick = () => {
		measure();
		if (performance.now() < pulseUntil) {
			pulseRaf = requestAnimationFrame(tick);
		} else {
			pulseRaf = undefined;
		}
	};
	pulseRaf = requestAnimationFrame(tick);
}

let spacePreserved = $state(false);

/** While set, the composer keeps an empty spacer in the keyboard's slot so the
 * input bar stays put while the keyboard is hidden under an overlay. */
export function preserveKeyboardSpace() {
	spacePreserved = true;
}

export function releaseKeyboardSpace() {
	spacePreserved = false;
}

export const keyboard = {
	/** Live keyboard height in px, following the open/close animation. */
	get height() {
		return liveHeight;
	},
	get isOpen() {
		return liveHeight > OPEN_THRESHOLD;
	},
	get spacePreserved() {
		return spacePreserved;
	},
	/** The full keyboard height (largest seen, persisted), or a fallback before
	 *  any keyboard has been shown. */
	get reservedHeight() {
		return maxHeight || FALLBACK_HEIGHT;
	},
};
