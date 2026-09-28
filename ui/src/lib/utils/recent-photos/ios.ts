import {
	type MediaItem,
	PHAssetCollectionSubtype,
	PHAssetCollectionType,
	PHAssetMediaType,
	PhotosAuthorizationStatus,
	getPhotosAuthStatus,
	requestAlbumMedias,
	requestAlbums,
	requestMediasByIds,
	requestPhotosAuth,
} from '@gbyte/tauri-plugin-ios-photos';
import { readFile, remove } from '@tauri-apps/plugin-fs';

import {
	type RecentPhoto,
	type RecentPhotosPermission,
	THUMBNAIL_PX,
} from './index';

/**
 * Long side, in pixels, requested when materializing a tapped photo for sending.
 * Large enough that any phone-camera photo comes back at native resolution
 * rather than the THUMBNAIL_PX strip size.
 */
const FULL_RES_PX = 1_000_000;

function toPermission(
	status: PhotosAuthorizationStatus | null,
): RecentPhotosPermission {
	if (
		status === PhotosAuthorizationStatus.authorized ||
		status === PhotosAuthorizationStatus.limited
	) {
		return 'granted';
	}
	if (status === PhotosAuthorizationStatus.notDetermined) return 'prompt';
	return 'denied';
}

export async function getPermission(): Promise<RecentPhotosPermission> {
	return toPermission(await getPhotosAuthStatus());
}

export async function requestPermission(): Promise<RecentPhotosPermission> {
	return toPermission(await requestPhotosAuth());
}

export async function listRecentPhotos(limit: number): Promise<RecentPhoto[]> {
	const albumId = await resolveAlbumId();
	if (!albumId) return [];
	// `limit` is applied natively (newest-first), so only that many assets are
	// rendered to thumbnails instead of the whole album. Videos are excluded
	// natively too: the plugin fully transcodes every video it returns.
	const medias = await requestAlbumMedias({
		id: albumId,
		width: THUMBNAIL_PX,
		height: THUMBNAIL_PX,
		quality: 0.7,
		limit,
		mediaType: PHAssetMediaType.image,
	});
	const recent = medias.filter(isImage).sort((a, b) => b.createAt - a.createAt);
	return Promise.all(recent.map(toRecentPhoto));
}

export async function loadPhotoBytes(id: string): Promise<Uint8Array> {
	// Render just this asset at full resolution by id — no whole-album re-render.
	const medias = await requestMediasByIds({
		ids: [id],
		width: FULL_RES_PX,
		height: FULL_RES_PX,
		quality: 1,
	});
	const match = medias.find(m => m.id === id && !!m.data);
	if (!match?.data) throw new Error('Photo no longer available');
	return readTempFile(match.data);
}

/** Reads a file the plugin rendered into the temp dir, then deletes it: the
 *  plugin writes a new file per request and never cleans them up. */
async function readTempFile(path: string): Promise<Uint8Array> {
	const bytes = await readFile(path);
	await remove(path).catch(e =>
		console.warn('Failed to remove rendered photo', e),
	);
	return bytes;
}

async function resolveAlbumId(): Promise<string | undefined> {
	const recent = await requestAlbums({
		with: PHAssetCollectionType.smartAlbum,
		subtype: PHAssetCollectionSubtype.smartAlbumRecentlyAdded,
	});
	if (recent[0]) return recent[0].id;
	const library = await requestAlbums({
		with: PHAssetCollectionType.smartAlbum,
		subtype: PHAssetCollectionSubtype.smartAlbumUserLibrary,
	});
	return library[0]?.id;
}

function isImage(item: MediaItem): item is MediaItem & { data: string } {
	return item.mediaType === PHAssetMediaType.image && !!item.data;
}

async function toRecentPhoto(
	item: MediaItem & { data: string },
	index: number,
): Promise<RecentPhoto> {
	const bytes = await readTempFile(item.data);
	return {
		id: item.id,
		thumbnail: new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }),
		name: `recent-${index}.jpg`,
		mimeType: 'image/jpeg',
	};
}
