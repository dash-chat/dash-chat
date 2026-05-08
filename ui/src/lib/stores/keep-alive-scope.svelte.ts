import { type ReactivePromise } from 'signalium';
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
