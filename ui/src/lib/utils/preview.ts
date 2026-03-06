/**
 * Utilities for the iframe-based preview deployment (Surge).
 *
 * When the app runs inside an iframe, the parent page hosts the toolbar and
 * sends commands via postMessage. This module translates those messages into
 * the same custom events the in-app PreviewToolbar dispatches, so the rest of
 * the app code stays unchanged.
 */

/** Whether the app is running inside an iframe (e.g. Surge preview wrapper). */
export const isInIframe = typeof window !== 'undefined' && window.self !== window.top;

/**
 * Start listening for postMessage commands from the parent iframe toolbar.
 * Returns a cleanup function that removes the listener.
 */
export function listenForPreviewMessages(): () => void {
	const handler = (event: MessageEvent) => {
		const data = event.data;
		if (!data || typeof data.type !== 'string') return;

		switch (data.type) {
			case 'theme-change':
				window.dispatchEvent(new CustomEvent('theme-change', { detail: data.payload }));
				break;
			case 'set-dark-mode':
				window.dispatchEvent(new CustomEvent('set-dark-mode', { detail: data.payload }));
				break;
			case 'set-wide-screen':
				window.dispatchEvent(new CustomEvent('set-wide-screen', { detail: data.payload }));
				break;
			case 'simulate-update':
				window.dispatchEvent(
					new CustomEvent('test-simulate-update', { detail: data.payload }),
				);
				break;
			case 'reset':
				localStorage.clear();
				window.location.reload();
				break;
			case 'wipe':
				localStorage.clear();
				localStorage.setItem('__preview_seeded', 'true');
				window.location.reload();
				break;
		}
	};
	window.addEventListener('message', handler);
	return () => window.removeEventListener('message', handler);
}
