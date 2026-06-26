import { isAndroid, isIos } from '$lib/utils/environment';
import {
	PHAssetCollectionSubtype,
	PHAssetCollectionType,
	PhotosAuthorizationStatus,
	createAlbum,
	createPhotos,
	requestAlbums,
	requestPhotosAuth,
} from '@gbyte/tauri-plugin-ios-photos';
import { invoke } from '@tauri-apps/api/core';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import type { FileAttachment, PhotoAttachment } from 'dash-chat-stores';
import { AndroidFs, AndroidPublicImageDir } from 'tauri-plugin-android-fs-api';
import { view } from 'tauri-plugin-view-api';

import { loadMediaBytes } from './media';

const GALLERY_ALBUM = 'Dash Chat';

/** Drop any directory components from a peer-supplied name so it can never
 * escape the directory we write it into. */
function basename(name: string): string {
	return name.split(/[\\/]/).pop() || 'attachment';
}

/**
 * Save a photo straight into the device gallery — the Pictures album via
 * MediaStore on Android, the photo library via the Photos framework on iOS —
 * so it shows up in the system gallery the way Signal does, with no share
 * sheet. Mobile-only; the caller handles desktop/browser saving.
 */
export async function savePhotoToGallery(
	photo: PhotoAttachment,
): Promise<void> {
	if (isAndroid) {
		const uri = await AndroidFs.createNewPublicImageFile(
			AndroidPublicImageDir.Pictures,
			`${GALLERY_ALBUM}/${basename(photo.name)}`,
			photo.mime_type,
		);
		await AndroidFs.writeFile(uri, await loadMediaBytes(photo));
	} else if (isIos) {
		await savePhotoToIosLibrary(photo);
	}
}

async function savePhotoToIosLibrary(photo: PhotoAttachment): Promise<void> {
	const status = await requestPhotosAuth();
	if (
		status !== PhotosAuthorizationStatus.authorized &&
		status !== PhotosAuthorizationStatus.limited
	) {
		throw new Error('Photo library access was denied');
	}
	const album = await getOrCreateAlbum(GALLERY_ALBUM);
	const path = await join(await appCacheDir(), basename(photo.name));
	await writeFile(path, await loadMediaBytes(photo));
	const created = await createPhotos({ album, files: [path] });
	if (!created) throw new Error('Failed to save photo to the library');
}

async function getOrCreateAlbum(title: string): Promise<string> {
	const albums = await requestAlbums({
		with: PHAssetCollectionType.album,
		subtype: PHAssetCollectionSubtype.albumRegular,
	});
	const existing = albums.find(a => a.name === title);
	if (existing) return existing.id;
	const id = await createAlbum({ title });
	if (!id) throw new Error('Failed to create gallery album');
	return id;
}

/**
 * Write a file attachment to the app cache and open it in the system viewer
 * (via FileProvider/ACTION_VIEW on Android, the document interaction menu on
 * iOS), matching Signal's tap-to-open behaviour with no share sheet.
 * Mobile-only; the caller handles desktop/browser saving.
 */
export async function saveAndOpenFile(file: FileAttachment): Promise<void> {
	const path = await invoke<string>('save_blob_to_cache', {
		hash: file.hash,
		name: file.name,
	});
	await view(path);
}
