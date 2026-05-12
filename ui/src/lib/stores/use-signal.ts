import { type ReactiveFn, ReactivePromise, watcher } from 'signalium';
import { type Readable } from 'svelte/store';

import { getKeepAliveScope } from './keep-alive-scope.svelte';

export function useSignal<T, Args extends unknown[]>(
	v: ReactiveFn<T, Args>,
	...args: Args
): Readable<T> {
	const w = watcher(() => {
		const value = v(...args);
		if (value instanceof ReactivePromise) {
			if (value.value !== undefined) return value.value;
		}
		return value;
	});
	return {
		subscribe: set => {
			const unsubs = w.addListener(() => {
				set(w.value);
			});
			set(w.value);
			return () => {
				unsubs();
			};
		},
	};
}

export function useReactivePromise<T, Args extends unknown[]>(
	v: (...args: Args) => ReactivePromise<T>,
	...args: Args
): Readable<Promise<T>> {
	// Register with the nearest KeepAliveScope (if any) so the signalium cache
	// for this (fn, args) is kept alive for the lifetime of the surrounding
	// route group — not just this consumer's subscription.
	getKeepAliveScope()?.keepAlive(v, args);

	const w = watcher(
		() => {
			const rp = v(...args);
			return { isReady: rp.isReady, value: rp.value, rp };
		},
		{
			equals: (prev, next) =>
				prev.isReady === next.isReady && Object.is(prev.value, next.value),
		},
	);

	return {
		subscribe: set => {
			// On first-load pending we expose the RP directly — Svelte's `{#await}`
			// awaits it via `rp.then()`. Once resolved we switch to value-keyed
			// `Promise.resolve(value)` references so Svelte re-renders only when
			// the value actually changes (refreshes with the same value are no-ops).
			let cachedPromise: Promise<T> | undefined;
			let lastValue: unknown;

			const emit = () => {
				const { isReady, value, rp } = w.value;

				if (isReady) {
					if (cachedPromise === (rp as unknown as Promise<T>)) {
						// The RP we exposed during pending just resolved — Svelte
						// already gets the value via `rp.then()`. Convert our
						// tracking promise to a value-keyed one for future
						// comparisons, but don't re-set (avoids a redundant
						// `:pending` flicker).
						lastValue = value;
						cachedPromise = Promise.resolve(value as T);
						return;
					}
					if (cachedPromise && Object.is(value, lastValue)) return;
					lastValue = value;
					cachedPromise = Promise.resolve(value as T);
					set(cachedPromise);
				} else if (cachedPromise) {
					// Refreshing with cached value — keep showing it.
					return;
				} else {
					cachedPromise = rp as unknown as Promise<T>;
					set(cachedPromise);
				}
			};

			const unsubs = w.addListener(emit);
			emit();
			return () => {
				unsubs();
			};
		},
	};
}
