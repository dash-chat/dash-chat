/** A step that waits for a toast to say something, for what the app only
 * reports after work of its own, like decoding an image. */
export function untilAToastShows(): Promise<void> {
	return new Promise(resolve => {
		const check = () => {
			const toast = document.querySelector('[data-testid="toast"]');
			if (toast !== null && (toast.textContent ?? '').trim() !== '') {
				resolve();
				return;
			}
			requestAnimationFrame(check);
		};
		check();
	});
}
