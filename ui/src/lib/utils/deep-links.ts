import { goto } from '$app/navigation';
import { onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(urls => {
		console.log('[deep-link] onOpenUrl fired, urls:', urls);
		for (const url of urls) {
			const match = url.match(/https:\/\/dashchat\.org\/add-contact\/(.+)/);
			if (match) {
				const code = match[1];
				console.log('[deep-link] matched, code:', code);
				goto('/new-message/add-contact').then(() =>
					showToast(`Got a deep link with code: ${code}`),
				);
			} else {
				console.log('[deep-link] url did not match pattern:', url);
			}
		}
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
