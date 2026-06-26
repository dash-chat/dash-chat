import type { FileAttachment, Hash, PhotoAttachment } from 'dash-chat-stores';

import { loadMediaBytes } from './media';

/**
 * Bytes of blob payloads kept resident as object URLs. Webviews don't cache
 * custom-protocol (`irohblob://`) responses — Android forces `no-store`, and
 * WKWebView never caches custom schemes — so without this cache every render
 * re-reads the blob from the node store over the IPC bridge. Content-addressed
 * hashes are immutable, so a cached URL never goes stale; this bound only caps
 * memory, with the least-recently-used entries evicted first.
 */
const MAX_CACHE_BYTES = 64 * 1024 * 1024;

interface CacheEntry {
	url: string;
	size: number;
}

// Map iteration order is insertion order, which doubles as LRU order: a cache
// hit re-inserts the entry so the oldest entry is always evicted first.
const cache = new Map<Hash, CacheEntry>();
const inFlight = new Map<Hash, Promise<string>>();
let cachedBytes = 0;

// Live <img src> references per blob: URL, plus the URLs that were dropped from
// the cache while still referenced. The cache is keyed by hash and can't know
// which URLs are bound to a mounted <img>, so eviction must not revoke a URL an
// off-screen-but-still-mounted image is using — that would surface a retry
// placeholder for an image whose bytes are fine. Instead the URL is pinned and
// revoked once {@link releaseMediaUrl} drops the last reference.
const urlRefs = new Map<string, number>();
const pinnedUrls = new Set<string>();

// Revoke an evicted entry's URL now, or pin it for later if a live <img> still
// references it.
function dropEntry(entry: CacheEntry): void {
	if ((urlRefs.get(entry.url) ?? 0) > 0) pinnedUrls.add(entry.url);
	else URL.revokeObjectURL(entry.url);
}

function evictToCap(): void {
	for (const [hash, entry] of cache) {
		if (cachedBytes <= MAX_CACHE_BYTES) break;
		cache.delete(hash);
		cachedBytes -= entry.size;
		dropEntry(entry);
	}
}

/** Mark a blob: URL as bound to a live <img> so eviction won't revoke it. */
export function acquireMediaUrl(url: string): void {
	urlRefs.set(url, (urlRefs.get(url) ?? 0) + 1);
}

/** Release a reference taken by {@link acquireMediaUrl}; revokes the URL once
 * the last referrer is gone and the entry was already evicted from the cache. */
export function releaseMediaUrl(url: string): void {
	const next = (urlRefs.get(url) ?? 0) - 1;
	if (next > 0) {
		urlRefs.set(url, next);
		return;
	}
	urlRefs.delete(url);
	if (pinnedUrls.delete(url)) URL.revokeObjectURL(url);
}

/**
 * Synchronously return the cached `blob:` URL for a hash whose bytes were
 * fetched on an earlier render, marking it most-recently-used. Returns
 * `undefined` on a miss, so the caller can fall back to loading via the
 * `irohblob://` scheme (and then populate the cache via {@link cacheMediaUrl}).
 */
export function cachedMediaUrl(hash: Hash): string | undefined {
	const hit = cache.get(hash);
	if (!hit) return undefined;
	cache.delete(hash);
	cache.set(hash, hit);
	return hit.url;
}

/**
 * Fetch a media item's bytes once and keep them as a `blob:` URL so later
 * renders — re-opening a chat, scrolling a photo back into view — reuse it with
 * no native round-trip. Resolves to the cached URL; concurrent calls for the
 * same hash share one fetch.
 */
export function cacheMediaUrl(
	item: PhotoAttachment | FileAttachment,
): Promise<string> {
	const existing = cachedMediaUrl(item.hash);
	if (existing) return Promise.resolve(existing);
	const pending = inFlight.get(item.hash);
	if (pending) return pending;

	const load = loadMediaBytes(item)
		.then(bytes => {
			const url = URL.createObjectURL(
				new Blob([bytes as BlobPart], { type: item.mime_type }),
			);
			cache.set(item.hash, { url, size: bytes.byteLength });
			cachedBytes += bytes.byteLength;
			evictToCap();
			return url;
		})
		.finally(() => inFlight.delete(item.hash));
	inFlight.set(item.hash, load);
	return load;
}

/**
 * Drop and revoke any cached URL for a hash so the next render re-fetches.
 * Used when a render failed — typically the blob hadn't synced yet — and the
 * user taps to retry.
 */
export function invalidateMediaUrl(hash: Hash): void {
	const entry = cache.get(hash);
	if (!entry) return;
	cache.delete(hash);
	cachedBytes -= entry.size;
	dropEntry(entry);
}
