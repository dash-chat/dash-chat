import { goto } from '$app/navigation';
import { eventually, find, settled } from 'i18n-screenshots/scene';

import { tid } from '../../../tests/selectors';

export { tid };

/** Go to `path` the way the app navigates, and wait for the page there. */
export async function open(path: string): Promise<void> {
	// A scene starts before the app can navigate or has its test hooks.
	await eventually(() => window.__test, 'the test hooks');
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
export async function untilAToastShows(): Promise<void> {
	await eventually(() => {
		const toast = document.querySelector(tid('toast'));
		return (toast?.textContent ?? '').trim() !== '' ? toast : undefined;
	}, 'a toast');
}
