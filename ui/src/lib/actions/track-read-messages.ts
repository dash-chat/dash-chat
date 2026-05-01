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
	const visible = new Set<Hash>();
	const ids = new WeakMap<Element, Hash>();
	let timer: ReturnType<typeof setTimeout> | undefined;

	const observer = new IntersectionObserver(
		entries => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				const id = ids.get(entry.target);
				if (id) visible.add(id);
			}
			clearTimeout(timer);
			timer = setTimeout(() => {
				if (visible.size === 0) return;
				store.markAsRead(Array.from(visible));
				visible.clear();
			}, debounceMs);
		},
		{ threshold },
	);

	const observe: Action<HTMLElement, Hash | null> = (node, id) => {
		if (id === null) return;
		ids.set(node, id);
		observer.observe(node);
		return {
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
		},
	};
}
