import type { Channel } from '@tauri-apps/api/core';
import { ReactivePromise, reactive, relay } from 'signalium';

interface TauriChannelInternals {
	id: number;
}

interface TauriInternals {
	unregisterCallback?(id: number): void;
}

declare global {
	interface Window {
		__TAURI_INTERNALS__?: TauriInternals;
	}
}

export function unregisterChannel<T>(channel: Channel<T>): void {
	if (typeof window === 'undefined') return;
	const id = (channel as unknown as TauriChannelInternals).id;
	window.__TAURI_INTERNALS__?.unregisterCallback?.(id);
}

export type UnsubscribeFn = () => void;

export function buildReactiveChannel<T, ARGS extends unknown[]>(
	fn: (handler: (v: T) => void, ...args: ARGS) => Promise<UnsubscribeFn>,
) {
	return reactive(
		(...args: ARGS): ReactivePromise<T> =>
			relay<T>(state => {
				let unsub: UnsubscribeFn | undefined;
				let cancelled = false;
				fn(
					v => {
						state.value = v;
					},
					...args,
				).then(u => {
					if (cancelled) u();
					else unsub = u;
				});
				return () => {
					cancelled = true;
					unsub?.();
				};
			}),
	);
}
