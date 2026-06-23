import { goto } from '$app/navigation';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

function handleAddContactLink(code: string) {
	console.log('[deep-link] handling add-contact link with code:', code);
	goto('/new-message/add-contact').then(() =>
		showToast(`Got a deep link with code: ${code}`),
	);
}

const HTTPS_DEEP_LINK_BASE_URL = 'https://dashchat\\.org';
const SCHEME_DEEP_LINK_BASE_URL = 'dash-chat:/';

function buildDeepLinkRegex(path: string): RegExp {
	return new RegExp(
		`^(?:${HTTPS_DEEP_LINK_BASE_URL}|${SCHEME_DEEP_LINK_BASE_URL})${path.replace(/\//g, '\\/')}$`,
	);
}

function handleUrls(urls: string[]) {
	console.log('[deep-link] handling urls:', urls);
	for (const url of urls) {
		const code = url.match(buildDeepLinkRegex('/add-contact/(.+)'))?.[1];
		if (code) {
			handleAddContactLink(code);
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
