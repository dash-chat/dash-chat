import { type ReactivePromise } from 'signalium';
import { getContext } from 'svelte';

export const SIGNAL_CACHE_KEY = Symbol('signal-cache');

export interface SignalCache {
	keepAlive<T, A extends unknown[]>(
		fn: (...a: A) => ReactivePromise<T>,
		args: A,
	): void;
}

export function getSignalCache(): SignalCache | undefined {
	return getContext(SIGNAL_CACHE_KEY);
}
