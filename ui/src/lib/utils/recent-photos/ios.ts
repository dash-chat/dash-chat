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

/**
 * Long side, in pixels, requested when materializing a tapped photo for sending.
 * The plugin (v0.3.0) renders assets to a target size and has no fetch-by-id
 * command, so a tap re-queries the album at this bound and picks the asset out
 * by id. Large enough that any phone-camera photo comes back at native
 * resolution rather than the THUMBNAIL_PX strip size.
 */
const FULL_RES_PX = 1_000_000;

/** Album resolved by the last listing, reused so a tap-to-send re-query skips
 * the album lookup. */
let albumIdCache: string | undefined;

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
	albumIdCache = albumId;
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
	const albumId = albumIdCache ?? (await resolveAlbumId());
	if (!albumId) throw new Error('Photo no longer available');
	// No fetch-by-id: re-render the album at full resolution and pick our asset.
	const medias = await requestAlbumMedias({
		id: albumId,
		width: FULL_RES_PX,
		height: FULL_RES_PX,
		quality: 1,
	});
	const match = medias.find(m => m.id === id && !!m.data);
	if (!match?.data) throw new Error('Photo no longer available');
	return readFile(match.data);
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
