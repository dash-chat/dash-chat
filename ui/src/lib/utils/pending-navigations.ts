import { goto } from '$app/navigation';
import { listen } from '@tauri-apps/api/event';
import { type ContactsStore, invokeAfterSetup } from 'dash-chat-stores';

import { handleUrls } from './deep-links';

type PendingNavigation =
	| { kind: 'deepLink'; target: string }
	| { kind: 'route'; target: string };

async function followNavigations(contactsStore: ContactsStore) {
	const navigations = await invokeAfterSetup<PendingNavigation[]>(
		'take_pending_navigations',
	);
	for (const navigation of navigations) {
		if (navigation.kind === 'deepLink') {
			handleUrls([navigation.target], contactsStore);
		} else {
			await goto(navigation.target);
		}
	}
}

/**
 * Follow the deep links and notification taps the OS delivers, including the
 * one that launched the app. Each is followed once: the backend hands every
 * navigation out a single time, so a webview reload doesn't replay it.
 */
export function followPendingNavigations(
	contactsStore: ContactsStore,
): () => void {
	const unlisten = listen('pending-navigations://new', () =>
		followNavigations(contactsStore),
	);
	unlisten
		.then(() => followNavigations(contactsStore))
		.catch(err => console.error('[navigation] failed to follow:', err));
	return () => {
		unlisten.then(fn => fn());
	};
}
