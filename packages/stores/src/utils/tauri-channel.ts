import type { Channel } from '@tauri-apps/api/core';

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
