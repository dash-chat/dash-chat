import * as addContact from '$lib/deep-links/add-contact';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

type DeepLinkHandler = {
	path: string;
	handle: (params: Record<string, string>) => void;
};

const handlers: DeepLinkHandler[] = [addContact];

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
	for (const url of urls) {
		let matched = false;
		for (const handler of handlers) {
			const params = matchesDeepLinkPath(url, handler.path);
			if (params) {
				handler.handle(params);
				matched = true;
				break;
			}
		}
		if (!matched) {
			console.log('[deep-link] url did not match pattern:', url);
			showToast(m.receivedUnrecognizedLink({ url }));
		}
	}
}

const handledLaunchUrls = new Set<string>();

export function handleLaunchDeepLink() {
	getCurrent()
		.then(urls => {
			if (urls) {
				for (const url of urls) handledLaunchUrls.add(url);
				handleUrls(urls);
			}
		})
		.catch(err => {
			console.error('[deep-link] failed to read launch deep links:', err);
		});
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(urls => {
		const fresh = urls.filter(url => !handledLaunchUrls.delete(url));
		if (fresh.length > 0) handleUrls(fresh);
	}).catch(err => {
		console.error('[deep-link] failed to register listener:', err);
		return () => {};
	});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
