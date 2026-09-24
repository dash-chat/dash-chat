import * as addContact from '$lib/deep-links/add-contact';
import { extractDeepLinkParams } from '$lib/deep-links/helpers';
import { m } from '$lib/paraglide/messages.js';
import type { ContactsStore } from 'dash-chat-stores';

import { showToast } from './toasts';

type DeepLinkHandler = {
	path: string;
	handle: (
		params: Record<string, string>,
		contactsStore: ContactsStore,
	) => Promise<void>;
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

export async function handleUrls(
	urls: string[],
	contactsStore: ContactsStore,
): Promise<void> {
	for (const url of urls) {
		let matched = false;
		for (const handler of handlers) {
			const params = extractDeepLinkParams(url, handler.path);
			if (params) {
				await handler.handle(params, contactsStore);
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
