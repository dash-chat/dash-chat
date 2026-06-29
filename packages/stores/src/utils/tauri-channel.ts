import { Channel } from '@tauri-apps/api/core';
import { type ReactivePromise, relay } from 'signalium';

import { invoke } from './invoke';

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

/// Bridge a backend streaming command (one that pushes updates over a Tauri
/// `Channel`) into a signalium `ReactivePromise`, unregistering the channel on
/// teardown.
export function subscribeChannel<T>(
	command: string,
	args: Record<string, unknown> = {},
): ReactivePromise<T> {
	return relay<T>(state => {
		const channel = new Channel<T>();
		channel.onmessage = v => {
			state.value = v;
		};
		invoke(command, { onEvent: channel, ...args });
		return () => unregisterChannel(channel);
	});
}
