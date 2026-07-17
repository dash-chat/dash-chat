import * as addContact from '$lib/deep-links/add-contact';
import { extractDeepLinkParams } from '$lib/deep-links/helpers';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';

import { showToast } from './toasts';

type DeepLinkHandler = {
	path: string;
	handle: (params: Record<string, string>) => void;
};

const handlers: DeepLinkHandler[] = [addContact];

function sanitizeUrl(url: string): string {
	try {
		const u = new URL(url);
		return `${u.protocol}//${u.host}`;
	} catch {
		return '(unparseable)';
	}
}

export function handleUrls(urls: string[]) {
	for (const url of urls) {
		let matched = false;
		for (const handler of handlers) {
			const params = extractDeepLinkParams(url, handler.path);
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

// URLs delivered at launch, populated before launchUrlsReady resolves.
// The onOpenUrl listener removes each URL on first sight so it only dedupes
// one re-delivery, not all future deliveries of the same URL.
const launchUrls = new Set<string>();

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
		if (urls.length > 0) handleUrls(urls);
	});
}

export function listenForDeepLinks(): () => void {
	const unlistenPromise = onOpenUrl(async urls => {
		await launchUrlsReady;
		// Remove each URL from launchUrls on first sight — if the plugin
		// re-delivers a launch URL, skip it exactly once; after that, treat
		// further deliveries as genuinely new.
		const fresh = urls.filter(url => !launchUrls.delete(url));
		if (fresh.length > 0) handleUrls(fresh);
	}).catch(err => {
		console.error('[deep-link] failed to register listener:', err);
		return () => {};
	});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
