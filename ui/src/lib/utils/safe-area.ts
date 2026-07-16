let maxBottom = 0;

function probeInsets(): { top: number; bottom: number } {
	const el = document.createElement('div');
	el.style.cssText =
		'position: fixed; visibility: hidden; top: env(safe-area-inset-top, 0px); bottom: env(safe-area-inset-bottom, 0px)';
	document.body.appendChild(el);
	const cs = getComputedStyle(el);
	const top = parseFloat(cs.top) || 0;
	const bottom = parseFloat(cs.bottom) || 0;
	el.remove();
	return { top, bottom };
}

/** Safe-area insets of the keyboard-closed layout, in px. The bottom inset
 * collapses to 0 while the soft keyboard is up (the webview's bottom edge
 * sits above it), so the largest value seen is reported instead of the
 * current one. */
export function safeAreaInsets(): { top: number; bottom: number } {
	const { top, bottom } = probeInsets();
	maxBottom = Math.max(maxBottom, bottom);
	return { top, bottom: maxBottom };
}

// Prime the remembered bottom inset at startup, before any keyboard can have
// collapsed it.
if (typeof document !== 'undefined' && document.body) {
	maxBottom = probeInsets().bottom;
}
