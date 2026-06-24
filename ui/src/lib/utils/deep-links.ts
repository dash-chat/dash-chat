import * as addContact from '$lib/deep-links/add-contact';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

type DeepLinkHandler = {
	path: string;
	handle: (params: Record<string, string>) => void;
};

const handlers: DeepLinkHandler[] = [addContact];

function escapeRegex(s: string): string {
	return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

const HTTPS_DEEP_LINK_BASE_URL = escapeRegex('https://dashchat.org');
const SCHEME_DEEP_LINK_BASE_URL = escapeRegex('dash-chat:/');

function matchesDeepLinkPath(
	url: string,
	path: string,
): Record<string, string> | null {
	const names: string[] = [];
	const pattern = path
		.split(/\{\{(\w+)\}\}/g)
		.map((chunk, i) => {
			if (i % 2 === 0) return escapeRegex(chunk);
			names.push(chunk);
			return '([^/?#]+)';
		})
		.join('');
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
			showToast(m.errorReceivedUnrecognizedLink({ url }), 'error');
		}
	}
}

const launchUrls = new Set<string>();
let launchUrlsConsumed = false;

// Resolved once getCurrent() completes so the onOpenUrl callback can safely
// dedup against launch URLs without a startup race.
const launchUrlsReady: Promise<void> = getCurrent()
	.then(urls => {
		if (urls) {
			for (const url of urls) launchUrls.add(url);
		}
	})
	.catch(err => {
		console.error('[deep-link] failed to read launch deep links:', err);
	});

export function handleLaunchDeepLink() {
	launchUrlsReady.then(() => {
		const urls = [...launchUrls];
		launchUrlsConsumed = true;
		if (urls.length > 0) handleUrls(urls);
	});
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(async urls => {
		await launchUrlsReady;
		// If the launch handler hasn't run yet, skip URLs it will handle.
		// Once it has run (or if it never will), treat everything as fresh.
		const fresh = launchUrlsConsumed
			? urls
			: urls.filter(url => !launchUrls.has(url));
		if (fresh.length > 0) handleUrls(fresh);
	}).catch(err => {
		console.error('[deep-link] failed to register listener:', err);
		return () => {};
	});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
