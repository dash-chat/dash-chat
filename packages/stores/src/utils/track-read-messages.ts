import type { Hash } from '../p2panda/types.js';

export interface ReadMessagesStore {
	readMessages(messageHashes: Hash[]): Promise<void>;
}

export interface TrackReadMessagesOptions {
	debounceMs?: number;
	threshold?: number;
}

type ObserveAction = (
	node: HTMLElement,
	id: Hash | null,
) => { destroy: () => void } | void;

export interface ReadMessagesTracker {
	observe: ObserveAction;
	destroy: () => void;
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
				store.readMessages(Array.from(visible));
				visible.clear();
			}, debounceMs);
		},
		{ threshold },
	);

	const observe: ObserveAction = (node, id) => {
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
