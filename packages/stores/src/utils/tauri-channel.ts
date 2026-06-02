import { Channel, invoke } from '@tauri-apps/api/core';

/**
 * Subscribe to a named source registered with `tauri-plugin-subscription`.
 * Returns an unsubscribe function that closes the resource (and cancels the
 * Rust-side forward task). Reload-safe — the plugin cancels all of a
 * webview's subscriptions on page reload.
 */
export function subscribeChannel<T>(
	name: string,
	args: Record<string, unknown>,
	onMessage: (value: T) => void,
): () => void {
	const channel = new Channel<T>();
	channel.onmessage = onMessage;
	const ridPromise = invoke<number>('plugin:subscription|subscribe', {
		name,
		args,
		channel,
	});
	return () => {
		ridPromise
			.then(rid => invoke('plugin:subscription|unsubscribe', { rid }))
			.catch(() => {
				/* preview build has no Tauri runtime; swallow */
			});
	};
}

export type UnsubscribeFn = () => void;
