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

/** Hands `url` to the deep link handler whose path it matches. Resolves to
 * whether one did. */
export async function handleDeepLink(
	url: string,
	contactsStore: ContactsStore,
): Promise<boolean> {
	for (const handler of handlers) {
		const params = extractDeepLinkParams(url, handler.path);
		if (params) {
			await handler.handle(params, contactsStore);
			return true;
		}
	}
	return false;
}

export async function handleUrls(
	urls: string[],
	contactsStore: ContactsStore,
): Promise<void> {
	for (const url of urls) {
		if (!(await handleDeepLink(url, contactsStore))) {
			console.log('[deep-link] url did not match pattern:', sanitizeUrl(url));
			showToast(m.errorReceivedUnrecognizedLink({ url }), 'error');
		}
	}
}
