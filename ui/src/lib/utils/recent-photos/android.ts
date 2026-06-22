import {
	type ImageInfo,
	MediaLibrarySource,
	SortColumn,
	SortDirection,
	getImages,
	requestPermissions,
} from '@universalappfactory/tauri-plugin-medialibrary';
import { AndroidFs } from 'tauri-plugin-android-fs-api';

import {
	type RecentPhoto,
	type RecentPhotosPermission,
	THUMBNAIL_PX,
} from './index';

function uri(contentUri: string) {
	return { uri: contentUri, documentTopTreeUri: null };
}

function imagesRequest(limit: number) {
	return {
		limit,
		offset: 0,
		source: MediaLibrarySource.ExternalStorage,
		sortColumn: SortColumn.DateAdded,
		sortDirection: SortDirection.Descending,
		includeFileMetadata: true,
	};
}

/** Whether the app currently has access to query MediaStore images. */
async function hasAccess(): Promise<boolean> {
	try {
		// The plugin misreports the granular permission state on Android 13+ (it
		// echoes the never-requested READ_EXTERNAL_STORAGE alias), so an actual
		// query is the only reliable proof of access: without the permission it
		// throws a SecurityException. A query that completes means access is
		// granted even when the gallery is empty, so we must not treat a zero-item
		// result as "no access".
		await getImages(imagesRequest(1));
		return true;
	} catch {
		return false;
	}
}

export async function getPermission(): Promise<RecentPhotosPermission> {
	// Before asking, an empty result means "not yet granted" → offer the button.
	return (await hasAccess()) ? 'granted' : 'prompt';
}

export async function requestPermission(): Promise<RecentPhotosPermission> {
	try {
		// Re-issue the OS permission request on every call; Android shows the
		// system dialog until the permission is permanently denied.
		await requestPermissions({ source: MediaLibrarySource.ExternalStorage });
	} catch (e) {
		console.error('Failed to request photo permission', e);
	}
	// Verify the outcome by probing actual access rather than trusting the
	// plugin's (Android 13+ unreliable) reported state.
	return (await hasAccess()) ? 'granted' : 'denied';
}

export async function listRecentPhotos(limit: number): Promise<RecentPhoto[]> {
	const result = await getImages(imagesRequest(limit));
	const items = result?.items ?? [];
	return Promise.all(items.map(toRecentPhoto));
}

export async function readPhotoBytes(contentUri: string): Promise<Uint8Array> {
	return AndroidFs.readFile(uri(contentUri));
}

async function toRecentPhoto(
	item: ImageInfo,
	index: number,
): Promise<RecentPhoto> {
	const dataUrl = await AndroidFs.getThumbnailAsDataURL(
		uri(item.contentUri),
		THUMBNAIL_PX,
		THUMBNAIL_PX,
	);
	return {
		id: item.contentUri,
		thumbnail: dataUrl ?? '',
		name: photoName(item, index),
		mimeType: item.mimeType || 'image/jpeg',
	};
}

function photoName(item: ImageInfo, index: number): string {
	const fromPath = item.path.split(/[\\/]/).pop();
	return fromPath || `recent-${index}`;
}
