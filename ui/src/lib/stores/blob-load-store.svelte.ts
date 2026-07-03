import type { Hash } from 'dash-chat-stores';

export type BlobLoadStatus = 'loading' | 'loaded' | 'error';

interface Entry {
	status: BlobLoadStatus;
	// 0 keeps the first load query-free (cacheable); a retry sets Date.now() so
	// the URL never reuses a cached failure, even across restarts.
	token: number;
}

const entries = $state<Record<Hash, Entry>>({});

/** Shared load status for a blob, tracked per content hash so every `BlobImage`
 * of the same blob agrees on whether it is loading, loaded, or errored. */
export function blobStatus(hash: Hash): BlobLoadStatus {
	return entries[hash]?.status ?? 'loading';
}

/** Cache-busting token for a blob's URL: 0 on first load, else the timestamp of
 * the latest retry. Shared so a retry from any surface re-fetches them all. */
export function blobToken(hash: Hash): number {
	return entries[hash]?.token ?? 0;
}

/** Record the outcome of an `<img>` load for a blob. Only `loaded`/`error` are
 * reported here; `loading` is owned by {@link retryBlob} and the initial state. */
export function reportBlobStatus(hash: Hash, status: BlobLoadStatus): void {
	const entry = entries[hash];
	if (entry) entry.status = status;
	else entries[hash] = { status, token: 0 };
}

/** Re-attempt a blob's download with a fresh cache-busting URL, moving every
 * `BlobImage` of that hash back to loading. */
export function retryBlob(hash: Hash): void {
	const token = Date.now();
	const entry = entries[hash];
	if (entry) {
		entry.status = 'loading';
		entry.token = token;
	} else {
		entries[hash] = { status: 'loading', token };
	}
}
