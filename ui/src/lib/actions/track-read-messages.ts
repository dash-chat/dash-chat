import type { Hash, MessagesStore } from 'dash-chat-stores';
import type { Action } from 'svelte/action';

interface TrackReadMessagesOptions {
	debounceMs?: number;
}

export interface ReadMessagesTracker {
	observe: Action<HTMLElement, Hash | null>;
	destroy(): void;
}

export function createReadMessagesTracker(
	store: MessagesStore,
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
		store
			.markAsRead(batch)
			.then(() => {
				retryDelay = debounceMs;
			})
			.catch(err => {
				if (destroyed) return;
				console.error('markAsRead failed, re-queuing hashes', err);
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
