import { type ReactiveFn, ReactivePromise, watcher } from 'signalium';
import { type Readable } from 'svelte/store';

export function useSignal<T, Args extends unknown[]>(
	v: ReactiveFn<T, Args>,
	...args: Args
): Readable<T> {
	const w = watcher(() => {
		const value = v(...args);
		const s = (value as any)['_signal'];
		const version = (value as any)['value'];

		if (value instanceof ReactivePromise && value.value !== undefined)
			return value.value;
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
	v: ReactiveFn<ReactivePromise<T>, Args>,
	...args: Args
): Readable<Promise<T>> {
	const w = watcher(
		() => {
			const rp = v(...args);
			// Entangle the watcher with the ReactivePromise's internal signal
			// and value so that state changes (pending→resolved) dirty the watcher.
			(rp as any)['_signal'];
			(rp as any)['value'];
			// Return a snapshot so equals() can compare actual state, not identity.
			return { isReady: rp.isReady, value: rp.value };
		},
		{
			// Only fire the listener when the RP's observable state actually changes
			// (ready flag or resolved value), not on every watcher re-evaluation.
			equals: (prev, next) =>
				prev.isReady === next.isReady && Object.is(prev.value, next.value),
		},
	);

	return {
		subscribe: set => {
			// Emit stable native Promise references so Svelte's {#await} works
			// correctly with signalium's ReactivePromise (RP).
			//
			// Problem: RP.then() queues callbacks when the Pending flag is set,
			// even during refresh when cached data is available (Pending | Ready).
			// This causes {#await} to re-enter the pending state. Additionally,
			// rapid re-evaluations can supersede pending promises via _setPromise,
			// leaving queued .then() callbacks permanently unresolved.
			//
			// Fix: never expose the raw RP to Svelte. Instead:
			// - When ready: emit Promise.resolve(value), cached by identity.
			// - When refreshing with cache: skip (keep current promise).
			// - When truly pending: emit a deferred Promise that we resolve
			//   ourselves when the watcher fires with ready data, bypassing
			//   RP.then() entirely.
			let cachedPromise: Promise<T> | undefined;
			let lastValue: unknown;
			let pendingResolve: ((value: T) => void) | undefined;

			const emit = () => {
				const { isReady, value: rpValue } = w.value;

				if (isReady) {
					const value = rpValue as T;

					if (pendingResolve) {
						// Resolve the deferred — Svelte's {#await} transitions to :then
						// without us calling set(), so Promise.all wrappers aren't
						// re-evaluated.
						pendingResolve(value);
						pendingResolve = undefined;
						lastValue = value;
						return;
					}

					if (cachedPromise && Object.is(value, lastValue)) {
						return; // same value, same promise — skip set()
					}
					lastValue = value;
					cachedPromise = Promise.resolve(value);
					set(cachedPromise);
				} else if (cachedPromise) {
					// RP is refreshing but we have cached data — keep showing it.
					return;
				} else if (!pendingResolve) {
					// Truly pending (first load) — create a deferred Promise that
					// we resolve via the watcher, avoiding RP.then() entirely.
					cachedPromise = new Promise<T>(resolve => {
						pendingResolve = resolve;
					});
					set(cachedPromise);
				}
			};

			const unsubs = w.addListener(emit);
			emit(); // synchronous initial value
			return () => {
				unsubs();
			};
		},
	};
}
