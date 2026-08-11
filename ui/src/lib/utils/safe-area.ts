let probe: HTMLElement | undefined;

// Safe-area insets in px, probed from `env()` — it has no JS accessor, and
// getComputedStyle hands back custom properties built on it (like
// `--keyboard-safe-bottom`) with their `max()` unresolved. The probe is created
// once and kept: callers read this on every scroll event, and creating and
// removing a node per read forces a style recalc each time.
export function safeAreaInsets(): { top: number; bottom: number } {
	if (!probe) {
		probe = document.createElement('div');
		probe.style.cssText =
			'position: fixed; visibility: hidden; pointer-events: none; top: env(safe-area-inset-top, 0px); bottom: env(safe-area-inset-bottom, 0px)';
		document.body.appendChild(probe);
	}
	const cs = getComputedStyle(probe);
	return { top: parseFloat(cs.top) || 0, bottom: parseFloat(cs.bottom) || 0 };
}
