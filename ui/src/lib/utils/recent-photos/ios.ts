import {
	type MediaItem,
	PHAssetCollectionSubtype,
	PHAssetCollectionType,
	PhotosAuthorizationStatus,
	getPhotosAuthStatus,
	requestAlbumMedias,
	requestAlbums,
	requestPhotosAuth,
} from '@gbyte/tauri-plugin-ios-photos';
import { readFile } from '@tauri-apps/plugin-fs';

import {
	type RecentPhoto,
	type RecentPhotosPermission,
	THUMBNAIL_PX,
} from './index';

/** PHAssetMediaType.image — the value iOS reports for still photos. */
const MEDIA_TYPE_IMAGE = 1;
/** Resolution requested when the user actually selects a photo to send; matches
 * the long-side the app compresses outgoing photos to. */
const FULL_PX = 1920;

let albumId: string | undefined;
const fullResPaths = new Map<string, string>();

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
	albumId = await resolveAlbumId();
	if (!albumId) return [];
	const medias = await requestAlbumMedias({
		id: albumId,
		width: THUMBNAIL_PX,
		height: THUMBNAIL_PX,
		quality: 0.7,
	});
	const recent = medias
		.filter(isImage)
		.sort((a, b) => b.createAt - a.createAt)
		.slice(0, limit);
	return Promise.all(recent.map(toRecentPhoto));
}

export async function loadPhotoBytes(id: string): Promise<Uint8Array> {
	let path = fullResPaths.get(id);
	if (!path) {
		await cacheFullResPaths();
		path = fullResPaths.get(id);
	}
	if (!path) throw new Error('Photo no longer available');
	return readFile(path);
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
	return item.mediaType === MEDIA_TYPE_IMAGE && !!item.data;
}

async function toRecentPhoto(
	item: MediaItem & { data: string },
	index: number,
): Promise<RecentPhoto> {
	const bytes = await readFile(item.data);
	return {
		id: item.id,
		thumbnail: new Blob([new Uint8Array(bytes)], { type: 'image/jpeg' }),
		name: `recent-${index}.jpg`,
		mimeType: 'image/jpeg',
	};
}

async function cacheFullResPaths(): Promise<void> {
	if (!albumId) albumId = await resolveAlbumId();
	if (!albumId) return;
	const medias = await requestAlbumMedias({
		id: albumId,
		width: FULL_PX,
		height: FULL_PX,
		quality: 0.9,
	});
	for (const item of medias) {
		if (isImage(item)) fullResPaths.set(item.id, item.data);
	}
}
