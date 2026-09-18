import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, it } from 'node:test';

import { BlobClient } from '../src/blobs/blob-client';
import type { BlobProgress } from '../src/blobs/blob-progress-tracker';

type TauriEvent = { event: string; id: number; payload: BlobProgress };

/** Stand in for the webview's Tauri bridge: `listen` registers its handler
 * through `transformCallback`, which we capture so a test can emit events. */
function installTauriStub(): (progress: BlobProgress) => void {
	let handler: ((e: TauriEvent) => void) | undefined;
	globalThis.window = {
		__TAURI_INTERNALS__: {
			invoke: async () => 1,
			transformCallback: (cb: (e: TauriEvent) => void) => {
				handler = cb;
				return 1;
			},
		},
	} as unknown as Window & typeof globalThis;
	return payload => {
		assert.ok(handler, 'listen was never called');
		handler({ event: 'blob://progress', id: 1, payload });
	};
}

describe('BlobClient.onBlobProgress', () => {
	let emit: (progress: BlobProgress) => void;
	let client: BlobClient;

	beforeEach(() => {
		emit = installTauriStub();
		client = new BlobClient();
	});

	afterEach(() => {
		delete (globalThis as { window?: unknown }).window;
	});

	it('delivers events for a hash to its subscriber', async () => {
		const seen: BlobProgress[] = [];
		client.onBlobProgress('h1', p => seen.push(p));
		await Promise.resolve();
		emit({ hash: 'h1', bytes: 10, complete: false });
		emit({ hash: 'h2', bytes: 20, complete: false });
		assert.deepEqual(seen, [{ hash: 'h1', bytes: 10, complete: false }]);
	});

	it('unsubscribing twice does not drop a later subscriber to the same hash', async () => {
		const unsubA = client.onBlobProgress('h1', () => {});
		unsubA();
		const seen: BlobProgress[] = [];
		client.onBlobProgress('h1', p => seen.push(p));
		unsubA();
		await Promise.resolve();
		emit({ hash: 'h1', bytes: 10, complete: false });
		assert.deepEqual(seen, [{ hash: 'h1', bytes: 10, complete: false }]);
	});
});
