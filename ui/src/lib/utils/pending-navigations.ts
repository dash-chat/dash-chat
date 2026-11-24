import { goto } from '$app/navigation';
import { listen } from '@tauri-apps/api/event';
import { type ContactsStore, invokeAfterSetup } from 'dash-chat-stores';

import { handleUrls } from './deep-links';

type PendingNavigation =
	| { kind: 'deepLink'; target: string }
	| { kind: 'route'; target: string };

async function followNavigation(
	navigation: PendingNavigation,
	contactsStore: ContactsStore,
) {
	if (navigation.kind === 'deepLink') {
		await handleUrls([navigation.target], contactsStore);
	} else {
		await goto(navigation.target);
	}
}

async function followNavigations(contactsStore: ContactsStore) {
	const navigations = await invokeAfterSetup<PendingNavigation[]>(
		'take_pending_navigations',
	);
	// Taking hands each navigation out once, so one that fails must not take
	// the rest with it.
	for (const navigation of navigations) {
		await followNavigation(navigation, contactsStore).catch(err =>
			console.error('[navigation] failed to follow', navigation.kind, err),
		);
	}
}

// Drains run one after another, so two of them can't interleave their
// navigations and leave the user on an arbitrary one.
let draining: Promise<void> = Promise.resolve();

function drain(contactsStore: ContactsStore) {
	draining = draining
		.then(() => followNavigations(contactsStore))
		.catch(err =>
			console.error('[navigation] failed to take pending navigations:', err),
		);
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
		drain(contactsStore),
	).catch(err => {
		console.error('[navigation] failed to listen for navigations:', err);
		return () => {};
	});
	unlisten.then(() => drain(contactsStore));
	return () => {
		unlisten.then(fn => fn());
	};
}
