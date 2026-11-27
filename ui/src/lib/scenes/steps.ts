import { goto } from '$app/navigation';
import { find, settled } from 'i18n-screenshots/scene';

import { tid } from '../../../tests/selectors';

export { tid };

function nextFrame(): Promise<void> {
	return new Promise(resolve => requestAnimationFrame(() => resolve()));
}

/** Go to `path` the way the app navigates, and wait for the page there. */
export async function open(path: string): Promise<void> {
	// A scene starts before the app can navigate or has its test hooks.
	while (window.__test === undefined) await nextFrame();
	await goto(path, { replaceState: true });
	// An entry of its own to go back to: a scene that goes back in history
	// would otherwise leave the app for the blank page the tab opened on.
	history.pushState(history.state, '', location.href);
	await settled();
}

/** Keep a finger on the element with `testId`, as a long-press does. */
export async function hold(testId: string): Promise<void> {
	await find(testId);
	await window.__test.longPress(tid(testId));
	await settled();
}

/** Waits for a toast to say something, for what the app only reports after
 * work of its own, like decoding an image. */
export function untilAToastShows(): Promise<void> {
	return new Promise(resolve => {
		const check = () => {
			const toast = document.querySelector(tid('toast'));
			if (toast !== null && (toast.textContent ?? '').trim() !== '') {
				resolve();
				return;
			}
			requestAnimationFrame(check);
		};
		check();
	});
}
