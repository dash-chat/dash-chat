import { type ReactivePromise, watcher } from 'signalium';
import { getContext, setContext } from 'svelte';

const KEY = Symbol('signal-cache');

type Entry = { unsub: () => void };

export interface SignalCache {
	keepAlive<T, A extends unknown[]>(
		fn: (...a: A) => ReactivePromise<T>,
		args: A,
	): void;
}

// Keeps signalium ReactivePromises cached while a layout is mounted, so child
// route swaps don't drop the last subscriber and re-resolve from scratch.
export function createSignalCache(): void {
	const cache = new Map<Function, Map<string, Entry>>();

	const scope: SignalCache = {
		keepAlive(fn, args) {
			let inner = cache.get(fn);
			if (!inner) {
				inner = new Map();
				cache.set(fn, inner);
			}
			const argsKey = args.length === 0 ? '' : JSON.stringify(args);
			if (inner.has(argsKey)) return;

			const w = watcher(() => {
				const rp = fn(...args);
				// Track _version like useReactivePromise does.
				(rp as any)['_version']?.['value'];
				return undefined;
			});
			inner.set(argsKey, { unsub: w.addListener(() => {}) });
		},
	};

	setContext(KEY, scope);

	$effect(() => () => {
		for (const inner of cache.values()) {
			for (const { unsub } of inner.values()) unsub();
		}
		cache.clear();
	});
}

export function getSignalCache(): SignalCache | undefined {
	return getContext(KEY);
}
