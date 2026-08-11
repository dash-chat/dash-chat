import {
	type Equals,
	type ReactiveFn,
	ReactivePromise,
	reactive,
	watcher,
} from 'signalium';
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

/**
 * Synchronously expose a signalium async reactive's resolved value as a Svelte
 * store. Emits `undefined` while the underlying ReactivePromise is pending,
 * then the resolved value. Use when you need to consume the value in plain
 * reactive expressions (`$derived`, class strings, …) rather than gating an
 * entire subtree behind `{#await}`.
 */
export function useReactiveValue<
	RP extends ReactivePromise<unknown>,
	Args extends unknown[],
>(v: (...args: Args) => RP, ...args: Args): Readable<Awaited<RP> | undefined> {
	type T = Awaited<RP>;
	const w = watcher(() => {
		const rp = v(...args);
		(rp as unknown as { _version: { value: unknown } })._version.value;
		return (rp.isReady ? rp.value : undefined) as T | undefined;
	});
	return {
		subscribe: set => {
			const read = () => set(w.value as T | undefined);
			const unsubs = w.addListener(read);
			read();
			return () => unsubs();
		},
	};
}

const STALLED_MS = 5_000;

/** The frame that called `useReactivePromise`, to name the wedged store in the log. */
function callSite(): string {
	const frames = new Error().stack?.split('\n').slice(1) ?? [];
	return (
		frames.find(frame => !frame.includes('use-signal'))?.trim() ?? 'unknown'
	);
}

export class StalledStoreError extends Error {
	constructor(origin: string) {
		super(
			`Store pending for ${STALLED_MS}ms without resolving, subscribed from ${origin}`,
		);
		this.name = 'StalledStoreError';
	}
}

export function useReactivePromise<
	RP extends ReactivePromise<unknown>,
	Args extends unknown[],
>(v: (...args: Args) => RP, ...args: Args): Readable<Promise<Awaited<RP>>> {
	type T = Awaited<RP>;
	// Register with the nearest KeepAliveScope (if any) so the signalium cache
	// for this (fn, args) is kept alive for the lifetime of the surrounding
	// route group — not just this consumer's subscription.
	getKeepAliveScope()?.keepAlive(v, args);

	const origin = callSite();

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
			const sentinel = Symbol('uninit');
			let lastEmittedValue: unknown = sentinel;
			let lastEmittedError: unknown = sentinel;
			let stalledTimer: ReturnType<typeof setTimeout> | undefined;
			const clearStalledTimer = () => {
				clearTimeout(stalledTimer);
				stalledTimer = undefined;
			};
			const emit = () => {
				const { isReady, isRejected, value, error } = w.value;
				if (isRejected) {
					// Surface the error so `{#await}` transitions to :catch.
					// Rejection takes precedence over a sticky `isReady` flag
					// from a prior successful resolution.
					if (Object.is(lastEmittedError, error) && lastEmittedSettled) return;
					clearStalledTimer();
					lastEmittedError = error;
					lastEmittedValue = sentinel;
					set(Promise.reject(error));
					lastEmittedSettled = true;
				} else if (isReady) {
					// signalium fires the watcher listener once on first attach
					// after the synchronous `w.value` read has already bumped
					// `updatedCount` past `listeners.updatedAt`, producing a
					// redundant fire with the same value. Dedupe by reference
					// so we don't hand Svelte a fresh Promise — that would
					// reset `{#await}` to :pending and unmount the :then branch.
					if (Object.is(lastEmittedValue, value) && lastEmittedSettled) return;
					clearStalledTimer();
					lastEmittedValue = value;
					lastEmittedError = sentinel;
					set(Promise.resolve(value as T));
					lastEmittedSettled = true;
				} else if (!lastEmittedSettled) {
					// First-load pending — emit a never-resolving placeholder so
					// {#await} stays in :pending until the first resolution.
					set(new Promise<T>(() => {}));
					if (stalledTimer === undefined) {
						stalledTimer = setTimeout(() => {
							stalledTimer = undefined;
							const error = new StalledStoreError(origin);
							// Also log: a consumer without a `{:catch}` arm would
							// otherwise swallow this into an unhandled rejection
							// that never reaches the tauri log.
							console.error(error.message);
							lastEmittedError = error;
							lastEmittedValue = sentinel;
							set(Promise.reject(error));
							lastEmittedSettled = true;
						}, STALLED_MS);
					}
				}
				// Else: a downstream recompute is in flight; keep showing the
				// previous value rather than flashing back to :pending.
			};
			const unsubs = w.addListener(emit);
			emit();
			return () => {
				clearStalledTimer();
				unsubs();
			};
		},
	};
}

/**
 * `useReactivePromise` for several async reactives at once: resolves to a tuple
 * of their values, so one `{#await}` — keeping its `:then`/`:catch` arms — can
 * gate a branch that needs all of them, instead of nesting an `{#await}` per
 * store.
 *
 *     useReactivePromises(() => [contactsStore.profiles(agentId), store.info()])
 */
export function useReactivePromises<
	const T extends readonly ReactivePromise<unknown>[],
>(
	sources: () => T,
): Readable<Promise<{ -readonly [K in keyof T]: Awaited<T[K]> }>> {
	type Values = { -readonly [K in keyof T]: Awaited<T[K]> };

	const sameValues = (a: readonly unknown[], b: readonly unknown[]) =>
		a.length === b.length && a.every((v, i) => Object.is(v, b[i]));

	// signalium runs an async reactive's `equals` against the resolved value but
	// types it against the promise.
	const equals = sameValues as unknown as Equals<Promise<Values>>;

	return useReactivePromise(
		reactive(async () => await ReactivePromise.all(sources()), { equals }),
	);
}
