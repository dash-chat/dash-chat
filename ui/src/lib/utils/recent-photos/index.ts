import { isAndroid, isIos, isTauriEnv } from '$lib/utils/environment';

import * as android from './android';
import * as ios from './ios';

/**
 * A device photo shown in the composer's recent-photos strip. `thumbnail` feeds
 * the `objectUrl` action: a `data:` URL string on Android, a `Blob` on iOS (the
 * action owns its object-URL lifecycle). `id` round-trips to
 * {@link loadRecentPhotoFile} to fetch the full-resolution bytes for sending.
 */
export interface RecentPhoto {
	id: string;
	thumbnail: Blob | string;
	name: string;
	mimeType: string;
}

export type RecentPhotosPermission = 'granted' | 'denied' | 'prompt';

/** Long side, in pixels, used for strip thumbnails on both platforms. */
export const THUMBNAIL_PX = 256;

/**
 * E2E seam: when `window.__test.recentPhotos` is set (see `RecentPhotosTestData`
 * in `ui/tests/setup-utils.ts`), the strip reads photos from there instead of
 * the native library, which is unavailable in the test harness. The shape is
 * inferred from the `window.__test` declaration so it lives in one place.
 */
function testData() {
	return typeof window !== 'undefined'
		? window.__test?.recentPhotos
		: undefined;
}

/** Whether reading recent photos is even possible in this environment. */
export const recentPhotosSupported = isTauriEnv() && (isIos || isAndroid);

/**
 * Current photo-library permission without triggering a system prompt where
 * possible. Returns `'prompt'` when access has not been decided yet.
 */
export async function getRecentPhotosPermission(): Promise<RecentPhotosPermission> {
	const test = testData();
	if (test) return test.permission;
	if (!recentPhotosSupported) return 'denied';
	if (isIos) return ios.getPermission();
	if (isAndroid) return android.getPermission();
	return 'denied';
}

/** Trigger the native permission prompt, returning the resulting permission. */
export async function requestRecentPhotosPermission(): Promise<RecentPhotosPermission> {
	const test = testData();
	if (test) return test.permission;
	if (isIos) return ios.requestPermission();
	if (isAndroid) return android.requestPermission();
	return 'denied';
}

/** Default number of recent photos to load for the strip. */
export const RECENT_PHOTOS_LIMIT = 24;

// First-paint cache only: overwritten by every listRecentPhotos call.
// RecentPhotosStrip re-queries on each panel open, so this never serves photos
// older than the previous open this session.
let cache: RecentPhoto[] | undefined;
let inFlight: Promise<RecentPhoto[]> | undefined;

async function fetchRecentPhotos(limit: number): Promise<RecentPhoto[]> {
	const test = testData();
	if (test) {
		return test.photos.slice(0, limit).map(p => ({
			id: p.id,
			thumbnail: p.dataUrl,
			name: p.name,
			mimeType: p.mimeType,
		}));
	}
	if (isIos) return ios.listRecentPhotos(limit);
	if (isAndroid) return android.listRecentPhotos(limit);
	return [];
}

/**
 * Recent photos, newest first, capped at `limit`. Photos only. Caches the
 * result and de-dupes concurrent calls (test data is never cached).
 */
export async function listRecentPhotos(
	limit: number = RECENT_PHOTOS_LIMIT,
): Promise<RecentPhoto[]> {
	if (testData()) return fetchRecentPhotos(limit);
	if (inFlight) return inFlight;
	inFlight = fetchRecentPhotos(limit);
	try {
		cache = await inFlight;
		return cache;
	} finally {
		inFlight = undefined;
	}
}

/** Last cached photos, read synchronously for instant display; `undefined`
 * until {@link listRecentPhotos} has run. */
export function cachedRecentPhotos(): RecentPhoto[] | undefined {
	return cache;
}

/** Full-resolution `File` for a tapped photo, ready for `ingestFiles`/`stage`. */
export async function loadRecentPhotoFile(photo: RecentPhoto): Promise<File> {
	if (testData()) {
		const src = photo.thumbnail;
		if (typeof src !== 'string') {
			return new File([src], photo.name, { type: photo.mimeType });
		}
		const res = await fetch(src);
		const bytes = new Uint8Array(await res.arrayBuffer());
		return new File([bytes], photo.name, { type: photo.mimeType });
	}
	const data = isIos
		? await ios.loadPhotoBytes(photo.id)
		: await android.readPhotoBytes(photo.id);
	return new File([new Uint8Array(data)], photo.name, { type: photo.mimeType });
}
