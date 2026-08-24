import * as addContact from '$lib/deep-links/add-contact';
import { extractDeepLinkParams } from '$lib/deep-links/helpers';
import { m } from '$lib/paraglide/messages.js';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';
import type { ContactsStore } from 'dash-chat-stores';

import { showToast } from './toasts';

type DeepLinkHandler = {
	path: string;
	handle: (
		params: Record<string, string>,
		contactsStore: ContactsStore,
	) => void;
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

export function handleUrls(urls: string[], contactsStore: ContactsStore) {
	for (const url of urls) {
		let matched = false;
		for (const handler of handlers) {
			const params = extractDeepLinkParams(url, handler.path);
			if (params) {
				handler.handle(params, contactsStore);
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

export function handleLaunchDeepLink(contactsStore: ContactsStore) {
	getCurrent()
		.then(urls => {
			if (urls && urls.length > 0) handleUrls(urls, contactsStore);
		})
		.catch(err => {
			console.error('[deep-link] failed to read launch deep links:', err);
		});
}

export function listenForDeepLinks(contactsStore: ContactsStore): () => void {
	const unlistenPromise = onOpenUrl(urls => {
		handleUrls(urls, contactsStore);
	}).catch(err => {
		console.error('[deep-link] failed to register listener:', err);
		return () => {};
	});

	return () => {
		unlistenPromise.then(fn => fn());
	};
}
