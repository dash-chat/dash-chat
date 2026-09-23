import { listen } from '@tauri-apps/api/event';
import type { BlobState, Hash, SystemEvent } from 'dash-chat-stores';
import { invokeAfterSetup } from 'dash-chat-stores';

/**
 * Whether each blob's bytes are on this device.
 *
 * The `irohblob://` scheme only ever serves blobs that are already here — it
 * cannot wait for one to arrive, because it answers inside the webview's
 * resource-interception callback, which every other request in the app queues
 * behind. So a surface asks here first, shows a placeholder while the answer is
 * `pending`, and points an `<img>` at the scheme only once the node reports the
 * bytes landed.
 */
const states = $state<Record<Hash, BlobState>>({});

let listening = false;

/** Start folding `BlobAvailable` events into the map. Idempotent, and never
 * torn down: the listener is per-app, not per-image. */
function listenForArrivals(): void {
	if (listening) return;
	listening = true;
	void listen('dashchat://system-event', event => {
		const system = event.payload as SystemEvent;
		if (system.type !== 'BlobAvailable') return;
		states[system.payload.hash] = 'local';
	});
}

/**
 * Ask the node for a blob and track what it says. Asking is what queues a fetch,
 * so this is called whenever a surface starts wanting the blob — including a
 * retry, where the point is to try again now rather than wait out the backoff
 * the previous failures earned.
 */
export async function requestBlob(hash: Hash): Promise<void> {
	listenForArrivals();
	// Re-asking for one already here would only confirm what we know, and the
	// arrival event keeps it true.
	if (states[hash] === 'local') return;
	states[hash] = await invokeAfterSetup<BlobState>('request_blob', { hash });
}

/** What the node last said about this blob, or `pending` until it has answered:
 * a surface should never show bytes it has not been told are here. */
export function blobState(hash: Hash): BlobState {
	return states[hash] ?? 'pending';
}
