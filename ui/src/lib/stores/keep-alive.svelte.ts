import { type ReactivePromise, watcher } from 'signalium';
import { getContext } from 'svelte';

export const KEEP_ALIVE_SCOPE_KEY = Symbol('keep-alive-scope');

export interface KeepAliveScope {
	keepAlive<T, A extends unknown[]>(
		fn: (...a: A) => ReactivePromise<T>,
		args: A,
	): void;
}

export function getKeepAliveScope(): KeepAliveScope | undefined {
	return getContext(KEEP_ALIVE_SCOPE_KEY);
}

/**
 * Touch signalium's private `_version` signal on a ReactivePromise so the
 * surrounding watcher is dirtied through the normal dependency graph on any
 * RP state change (pending → resolved, resolved → refreshing, etc.).
 */
export function trackRpVersion(rp: ReactivePromise<unknown>): void {
	(rp as { _version?: { value: unknown } })._version?.value;
}

/**
 * Keep a signalium reactive subscribed for the lifetime of the calling
 * component, so its cached value (and the values of its dependencies)
 * survives child route navigation.
 */
export function useKeepAlive<T, A extends unknown[]>(
	fn: (...args: A) => ReactivePromise<T>,
	...args: A
): void {
	$effect(() => {
		const w = watcher(() => {
			trackRpVersion(fn(...args));
			return undefined;
		});
		const unsub = w.addListener(() => {});
		return () => unsub();
	});
}
