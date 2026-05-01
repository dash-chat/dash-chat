/**
 * Returns a keydown handler that fires `callback` on Enter or Space,
 * calling `e.preventDefault()` to suppress the browser's default scroll.
 */
export function onActivate(callback: () => void) {
	return (e: KeyboardEvent) => {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			callback();
		}
	};
}
