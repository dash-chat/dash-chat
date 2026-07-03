import type { Hash } from 'dash-chat-stores';

interface Entry {
	// 0 keeps the first load query-free (cacheable); a retry sets Date.now() so
	// the URL never reuses a cached failure, even across restarts.
	token: number;
	// Number of mounted BlobImages of this hash. The entry is dropped when it
	// reaches 0 so the map stays bounded to blobs currently on screen and a later
	// remount starts a fresh load rather than reusing a stale retry token.
	refs: number;
}

const entries = $state<Record<Hash, Entry>>({});

function entryFor(hash: Hash): Entry {
	let entry = entries[hash];
	if (!entry) {
		entry = { token: 0, refs: 0 };
		entries[hash] = entry;
	}
	return entry;
}

/** Register a mounted `BlobImage` for this hash; pair with {@link releaseBlob}. */
export function acquireBlob(hash: Hash): void {
	entryFor(hash).refs += 1;
}

/** Unregister a `BlobImage`, dropping the entry once no surface references it. */
export function releaseBlob(hash: Hash): void {
	const entry = entries[hash];
	if (!entry) return;
	entry.refs -= 1;
	if (entry.refs <= 0) delete entries[hash];
}

/** Cache-busting token for a blob's URL: 0 until a retry, then the timestamp of
 * the latest one. Shared per hash so a retry from any surface re-fetches the
 * blob on every mounted surface at once. */
export function blobToken(hash: Hash): number {
	return entries[hash]?.token ?? 0;
}

/** Re-attempt a blob's download with a fresh cache-busting URL, moving every
 * mounted `BlobImage` of that hash back to loading. */
export function retryBlob(hash: Hash): void {
	entryFor(hash).token = Date.now();
}
