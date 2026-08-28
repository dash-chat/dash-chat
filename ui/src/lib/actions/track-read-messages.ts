import type { DeviceId, Hash, Message, MessagesStore } from 'dash-chat-stores';
import type { Action } from 'svelte/action';

interface TrackReadMessagesOptions {
	debounceMs?: number;
}

export interface ReadMessagesTracker {
	observe: Action<HTMLElement, Hash | null>;
	destroy(): void;
}

/** Chronological display order of two messages: timestamp, hash as
 * tiebreak — the same ordering the message list renders in. */
function messageOrder(a: Message, b: Message): number {
	return a.timestamp - b.timestamp || a.hash.localeCompare(b.hash);
}

/** Marks the seen messages as read, together with every earlier unread
 * message from other devices — seeing a message implies having seen
 * everything before it in the conversation. */
async function markSeenAndEarlierAsRead(
	store: MessagesStore,
	me: DeviceId,
	seen: Hash[],
): Promise<void> {
	const messages = await store.messages();
	const readHashes = await store.readMessageHashes();

	let latest: Message | undefined;
	for (const hash of seen) {
		const message = messages[hash];
		if (message && (!latest || messageOrder(latest, message) < 0)) {
			latest = message;
		}
	}

	const toMark = new Set(seen);
	if (latest) {
		for (const message of Object.values(messages)) {
			if (message.author === me) continue;
			if (readHashes.has(message.hash)) continue;
			if (messageOrder(message, latest) <= 0) toMark.add(message.hash);
		}
	}
	await store.markAsRead(Array.from(toMark));
}

export function createReadMessagesTracker(
	store: MessagesStore,
	myDeviceId: DeviceId,
	options: TrackReadMessagesOptions = {},
): ReadMessagesTracker {
	const { debounceMs = 500 } = options;
	const maxRetryDelayMs = 30_000;
	const visible = new Set<Hash>();
	const ids = new WeakMap<Element, Hash>();
	let timer: ReturnType<typeof setTimeout> | undefined;
	let retryDelay = debounceMs;
	let destroyed = false;

	const flush = () => {
		if (destroyed || visible.size === 0) return;
		const batch = Array.from(visible);
		visible.clear();
		markSeenAndEarlierAsRead(store, myDeviceId, batch)
			.then(() => {
				retryDelay = debounceMs;
			})
			.catch(err => {
				if (destroyed) return;
				console.error('marking messages read failed, re-queuing hashes', err);
				for (const hash of batch) visible.add(hash);
				retryDelay = Math.min(retryDelay * 2, maxRetryDelayMs);
				clearTimeout(timer);
				timer = setTimeout(flush, retryDelay);
			});
	};

	const observer = new IntersectionObserver(entries => {
		for (const entry of entries) {
			if (!entry.isIntersecting) continue;
			const id = ids.get(entry.target);
			if (id !== undefined) visible.add(id);
		}
		clearTimeout(timer);
		timer = setTimeout(flush, debounceMs);
	});

	// A message counts as read once its bottom edge scrolls into view
	const observe: Action<HTMLElement, Hash | null> = (node, id) => {
		const sentinel = document.createElement('div');
		sentinel.style.height = '1px';
		sentinel.style.marginTop = '-1px';
		sentinel.style.pointerEvents = 'none';
		node.appendChild(sentinel);
		if (id !== null) {
			ids.set(sentinel, id);
			observer.observe(sentinel);
		}
		return {
			update(newId: Hash | null) {
				if (newId === null) {
					observer.unobserve(sentinel);
					ids.delete(sentinel);
				} else if (ids.get(sentinel) !== newId) {
					ids.set(sentinel, newId);
					observer.observe(sentinel);
				}
			},
			destroy() {
				observer.unobserve(sentinel);
				ids.delete(sentinel);
				sentinel.remove();
			},
		};
	};

	return {
		observe,
		destroy() {
			clearTimeout(timer);
			observer.disconnect();
			flush();
			destroyed = true;
		},
	};
}
