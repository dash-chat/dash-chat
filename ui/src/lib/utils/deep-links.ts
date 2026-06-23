import { goto } from '$app/navigation';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

const DEEP_LINK_BASE_URL = 'https://dashchat.org';

function handleAddContactLink(code: string) {
	console.log('[deep-link] handling add-contact link with code:', code);
	goto('/new-message/add-contact').then(() =>
		showToast(`Got a deep link with code: ${code}`),
	);
}

function handleUrls(urls: string[]) {
	console.log('[deep-link] handling urls:', urls);
	for (const url of urls) {
		const match = url.match(
			new RegExp(`${DEEP_LINK_BASE_URL}/add-contact/(.+)`),
		);
		if (match) {
			const code = match[1];
			handleAddContactLink(code);
		} else {
			console.log('[deep-link] url did not match pattern:', url);
		}
	}
}

export async function handleLaunchDeepLink() {
	const urls = await getCurrent();
	if (urls) handleUrls(urls);
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(urls => {
		console.log('[deep-link] onOpenUrl fired, urls:', urls);
		handleUrls(urls);
	})
		.then(fn => fn)
		.catch(err => {
			console.error('[deep-link] failed to register listener:', err);
			return () => {};
		});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
