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
 * Keep a signalium reactive subscribed for the lifetime of the calling
 * component, so its cached value (and the values of its dependencies)
 * survives child route navigation.
 */
export function useKeepAlive<T, A extends unknown[]>(
	fn: (...args: A) => ReactivePromise<T>,
	...args: A
): void {
	const w = watcher(() => {
		const rp = fn(...args);
		(rp as any)['_version']?.['value'];
		return undefined;
	});
	const unsub = w.addListener(() => {});
	$effect(() => () => unsub());
}
