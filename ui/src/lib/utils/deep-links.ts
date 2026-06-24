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
			`^(?:${HTTPS_DEEP_LINK_BASE_URL}|${SCHEME_DEEP_LINK_BASE_URL})${pattern}(?:[?#].*)?$`,
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

function sanitizeUrl(url: string): string {
	try {
		const u = new URL(url);
		return `${u.protocol}//${u.host}`;
	} catch {
		return '(unparseable)';
	}
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
			console.log('[deep-link] url did not match pattern:', sanitizeUrl(url));
			showToast(m.errorReceivedUnrecognizedLink({ url }));
		}
	}
}

const handledLaunchUrls = new Set<string>();

// Resolved once getCurrent() completes so the onOpenUrl callback can safely
// dedup against launch URLs without a startup race.
const launchUrlsReady: Promise<void> = getCurrent()
	.then(urls => {
		if (urls) {
			for (const url of urls) handledLaunchUrls.add(url);
		}
	})
	.catch(err => {
		console.error('[deep-link] failed to read launch deep links:', err);
	});

export function handleLaunchDeepLink() {
	launchUrlsReady.then(() => {
		const urls = [...handledLaunchUrls];
		if (urls.length > 0) handleUrls(urls);
	});
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(async urls => {
		await launchUrlsReady;
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
