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
			// Track _version so async reactive re-runs (which reuse the RP via
			// _setPromise without bumping the signal's updatedCount) still
			// dirty the watcher. Mirrors the tracking in useReactivePromise.
			(value as unknown as { _version: { value: unknown } })._version.value;
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
			// Track the RP's _version signal so the watcher is dirtied on every
			// RP state transition. Async reactives reuse the same RP across
			// re-runs (via _setPromise) without incrementing the signal's
			// updatedCount, so the signal-level edge check sees no change.
			// _version is bumped on every _setFlags call (pending, resolved,
			// new value), which propagates dirty-ness through signalium's
			// normal dependency graph.
			(rp as unknown as { _version: { value: unknown } })._version.value;
			return {
				isReady: rp.isReady,
				isRejected: rp.isRejected,
				value: rp.value,
				error: rp.error,
			};
		},
		{
			equals: (prev, next) =>
				prev.isReady === next.isReady &&
				prev.isRejected === next.isRejected &&
				Object.is(prev.value, next.value) &&
				Object.is(prev.error, next.error),
		},
	);

	return {
		subscribe: set => {
			let lastEmittedSettled = false;
			const emit = () => {
				const { isReady, isRejected, value, error } = w.value;
				if (isRejected) {
					// Surface the error so `{#await}` transitions to :catch.
					// Rejection takes precedence over a sticky `isReady` flag
					// from a prior successful resolution.
					set(Promise.reject(error));
					lastEmittedSettled = true;
				} else if (isReady) {
					// Always hand Svelte a freshly-resolved Promise so `{#await}`
					// transitions to the :then branch with the new value.
					set(Promise.resolve(value as T));
					lastEmittedSettled = true;
				} else if (!lastEmittedSettled) {
					// First-load pending — emit a never-resolving placeholder so
					// {#await} stays in :pending until the first resolution.
					set(new Promise<T>(() => {}));
				}
				// Else: a downstream recompute is in flight; keep showing the
				// previous value rather than flashing back to :pending.
			};
			const unsubs = w.addListener(emit);
			emit();
			return () => {
				unsubs();
			};
		},
	};
}
