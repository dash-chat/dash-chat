import type { Hash, ReadMessagesStore } from 'dash-chat-stores';
import type { Action } from 'svelte/action';

interface TrackReadMessagesOptions {
	debounceMs?: number;
	threshold?: number;
}

export interface ReadMessagesTracker {
	observe: Action<HTMLElement, Hash | null>;
	destroy(): void;
}

export function createReadMessagesTracker(
	store: ReadMessagesStore,
	options: TrackReadMessagesOptions = {},
): ReadMessagesTracker {
	const { debounceMs = 500, threshold = 0.5 } = options;
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

	const observer = new IntersectionObserver(
		entries => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				const id = ids.get(entry.target);
				if (id !== undefined) visible.add(id);
			}
			clearTimeout(timer);
			timer = setTimeout(flush, debounceMs);
		},
		{ threshold },
	);

	const observe: Action<HTMLElement, Hash | null> = (node, id) => {
		if (id !== null) {
			ids.set(node, id);
			observer.observe(node);
		}
		return {
			update(newId: Hash | null) {
				if (newId === null) {
					observer.unobserve(node);
					ids.delete(node);
				} else if (ids.get(node) !== newId) {
					ids.set(node, newId);
					observer.observe(node);
				}
			},
			destroy() {
				observer.unobserve(node);
				ids.delete(node);
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
