import { goto } from '$app/navigation';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

function handleAddContactLink(code: string) {
	console.log('[deep-link] handling add-contact link with code:', code);
	goto('/new-message/add-contact').then(() =>
		// TODO: This is temporary until another PR actually uses the code to add
		//       the contact (hence why it doesn't use paraglide messages)
		showToast(`Got a deep link with code: ${code}`),
	);
}

const HTTPS_DEEP_LINK_BASE_URL = 'https://dashchat\\.org';
const SCHEME_DEEP_LINK_BASE_URL = 'dash-chat:/';

function matchesDeepLinkPath(
	url: string,
	path: string,
): Record<string, string> | null {
	const names: string[] = [];
	const pattern = path
		.replace(/\//g, '\\/')
		.replace(/\{\{(\w+)\}\}/g, (_, name: string) => {
			names.push(name);
			return '([^\\/?#]+)';
		});
	const match = url.match(
		new RegExp(
			`^(?:${HTTPS_DEEP_LINK_BASE_URL}|${SCHEME_DEEP_LINK_BASE_URL})${pattern}$`,
		),
	);
	if (!match) return null;
	return Object.fromEntries(
		names.map((name, i) => {
			try {
				return [name, decodeURIComponent(match[i + 1])];
			} catch {
				return [name, match[i + 1]];
			}
		}),
	);
}

function handleUrls(urls: string[]) {
	console.log('[deep-link] handling urls:', urls);
	for (const url of urls) {
		const match = matchesDeepLinkPath(url, '/add-contact/{{code}}');
		if (match?.code) {
			handleAddContactLink(match.code);
		} else {
			console.log('[deep-link] url did not match pattern:', url);
			showToast(m.receivedUnrecognizedLink({ url }));
		}
	}
}

export function handleLaunchDeepLink() {
	getCurrent()
		.then(urls => {
			if (urls) handleUrls(urls);
		})
		.catch(err => {
			console.error('[deep-link] failed to read launch deep links:', err);
		});
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(urls => {
		console.log('[deep-link] onOpenUrl fired, urls:', urls);
		handleUrls(urls);
	}).catch(err => {
		console.error('[deep-link] failed to register listener:', err);
		return () => {};
	});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
