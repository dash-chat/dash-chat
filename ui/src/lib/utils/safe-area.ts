// Safe-area insets in px, probed from `env()`
export function safeAreaInsets(): { top: number; bottom: number } {
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
