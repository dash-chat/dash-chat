import { m } from '$lib/paraglide/messages.js';
import { compressImage } from '$lib/utils/compress';
import { isIos, isMobile, isTauriEnv } from '$lib/utils/environment';
import {
	openFileInput,
	pickFiles,
	pickNativeFiles,
	saveFile,
} from '$lib/utils/files';
import { saveAndOpenFile, savePhotoToGallery } from '$lib/utils/gallery';
import { convertFileSrc } from '@tauri-apps/api/core';
import { downloadDir } from '@tauri-apps/api/path';
import type {
	FileAttachment,
	Hash,
	OutgoingFile,
	OutgoingMedia,
	OutgoingPhoto,
	PhotoAttachment,
} from 'dash-chat-stores';

/**
 * Draft voice note held in the composer before sending.
 */
export interface DraftVoiceNote {
	bytes: Uint8Array;
	mimeType: string;
	durationMs: number;
	waveform: Uint8Array;
}

export const MAX_MESSAGE_BYTES = 16 * 1024 * 1024;

export class AttachmentTooLargeError extends Error {
	readonly totalBytes: number;
	readonly maxBytes: number;
	constructor(totalBytes: number, maxBytes: number = MAX_MESSAGE_BYTES) {
		super(`Attachments total ${totalBytes} bytes, exceeds ${maxBytes}`);
		this.name = 'AttachmentTooLargeError';
		this.totalBytes = totalBytes;
		this.maxBytes = maxBytes;
	}
}

/**
 * Draft media held in the composer before sending — raw `File` refs. Previews
 * derive their object URL via the `objectUrl` action on the `<img>`, so the
 * draft carries no URLs that need revoking.
 */
export type DraftMedia =
	| { kind: 'photos'; items: File[] }
	| { kind: 'file'; file: File }
	| { kind: 'voice_note'; voice: DraftVoiceNote };

export const MAX_STAGED_PHOTOS = 32;

export type IngestError = 'tooMany' | 'filesWithPhotos' | 'oneFileAtATime';

export interface IngestResult {
	media: DraftMedia | undefined;
	error?: IngestError;
}

// Only raster types that decode in every supported webview may enter the
// photo path; anything else (videos until <video> rendering exists, and
// formats with patchy decoder support like HEIC/AVIF/TIFF) stages and
// sends as a file attachment instead of producing a broken <img> on the
// recipient.
const PHOTO_TYPES = new Set([
	'image/jpeg',
	'image/png',
	'image/webp',
	'image/gif',
]);

/** `accept` value for photo pickers, kept in sync with the photo path so a
 * "Photos" pick can't surface file-attachment error toasts. */
export const PHOTO_ACCEPT = [...PHOTO_TYPES].join(',');

/**
 * Pick media for the composer. On iOS the native picker opens the photo library
 * (PHPickerViewController) or the document browser directly; a web `<input
 * type=file>` would instead show a Photo Library / Take Photo / Choose File
 * action sheet. Everywhere else the web input already opens the right picker, so
 * it keeps using it. Resolves with the chosen files, or `null` if dismissed.
 */
export async function pickMedia(
	mode: 'image' | 'document',
	multiple: boolean,
): Promise<File[] | null> {
	if (isIos && isTauriEnv()) {
		return pickNativeFiles({ mode, multiple });
	}
	const accept = mode === 'image' ? PHOTO_ACCEPT : undefined;
	const list = await pickFiles({ accept, multiple });
	return list ? Array.from(list) : null;
}

/**
 * Take a photo with the device camera, resolving with it or `null` if the user
 * backed out.
 */
export async function capturePhoto(): Promise<File | null> {
	const list = await openFileInput({ accept: 'image/*', capture: true });
	return list?.[0] ?? null;
}

function isVisualFile(file: File): boolean {
	return PHOTO_TYPES.has(file.type);
}

/**
 * Add `files` to the current draft following Signal's mixing rules: photos
 * append (up to `MAX_STAGED_PHOTOS`, accepting a partial batch), a
 * non-image file can only be staged alone, and nothing can be added once
 * a file is staged. On a rule violation the current draft is returned
 * unchanged alongside the error.
 */
export function ingestFiles(
	current: DraftMedia | undefined,
	files: File[],
): IngestResult {
	if (files.length === 0) return { media: current };
	if (current?.kind === 'file') {
		return { media: current, error: 'oneFileAtATime' };
	}
	const visual = files.filter(isVisualFile);
	const nonVisual = files.filter(f => !isVisualFile(f));
	if (nonVisual.length > 0) {
		if (current || visual.length > 0) {
			return { media: current, error: 'filesWithPhotos' };
		}
		if (nonVisual.length > 1) {
			return { media: current, error: 'oneFileAtATime' };
		}
		return { media: { kind: 'file', file: nonVisual[0] } };
	}
	const existing = current?.kind === 'photos' ? current.items : [];
	const room = MAX_STAGED_PHOTOS - existing.length;
	if (room <= 0) return { media: current, error: 'tooMany' };
	const accepted = visual.slice(0, room);
	const items = [...existing, ...accepted];
	return {
		media: { kind: 'photos', items },
		error: visual.length > room ? 'tooMany' : undefined,
	};
}

/**
 * Convert composer-side draft to the wire-format `Media` for sending.
 * Compresses images first, then enforces a total-size cap; throws
 * `AttachmentTooLargeError` if the post-compression payload still exceeds it.
 */
export async function draftToMedia(draft: DraftMedia): Promise<OutgoingMedia> {
	const media = await buildMedia(draft);
	const total = totalMediaBytes(media);
	if (total > MAX_MESSAGE_BYTES) {
		throw new AttachmentTooLargeError(total);
	}
	return media;
}

async function buildMedia(draft: DraftMedia): Promise<OutgoingMedia> {
	if (draft.kind === 'photos') {
		const photos: OutgoingPhoto[] = await Promise.all(
			draft.items.map(async file => {
				const compressed = await compressImage(file);
				return {
					data: new Uint8Array(await compressed.arrayBuffer()),
					name: compressed.name,
					mime_type: compressed.type || 'application/octet-stream',
				};
			}),
		);
		return { kind: 'photos', photos };
	}
	if (draft.kind === 'voice_note') {
		return {
			kind: 'voice_note',
			voice_note: {
				data: draft.voice.bytes,
				mime_type: draft.voice.mimeType,
				duration_ms: draft.voice.durationMs,
				waveform: draft.voice.waveform,
			},
		};
	}
	const file: OutgoingFile = {
		data: new Uint8Array(await draft.file.arrayBuffer()),
		name: draft.file.name,
		mime_type: draft.file.type || 'application/octet-stream',
	};
	return { kind: 'file', file };
}

function totalMediaBytes(media: OutgoingMedia): number {
	if (media.kind === 'photos') {
		return media.photos.reduce((sum, p) => sum + p.data.byteLength, 0);
	}
	if (media.kind === 'voice_note') {
		return media.voice_note.data.byteLength;
	}
	return media.file.data.byteLength;
}

/** Uppercase extension for a filename, max 4 chars; '' when there is none. */
export function fileExtension(name: string): string {
	const dot = name.lastIndexOf('.');
	if (dot <= 0 || dot === name.length - 1) return '';
	return name
		.slice(dot + 1)
		.toUpperCase()
		.slice(0, 4);
}

export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1024 * 1024 * 1024)
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/**
 * Save a photo: straight to the device gallery on mobile (Pictures via
 * MediaStore on Android, the photo library on iOS), a native save dialog on
 * desktop Tauri, or an anchor-download in the browser. Returns `true` when it
 * was saved (so the caller can confirm with a toast) and `false` when the user
 * cancelled the desktop dialog. Throws on unexpected failure.
 */
export async function savePhoto(photo: PhotoAttachment): Promise<boolean> {
	if (isTauriEnv() && isMobile) {
		await savePhotoToGallery(photo);
		return true;
	}
	return saveToDisk(photo);
}

/**
 * Save a file attachment: saved to the app storage and opened with the system
 * handler on mobile, a native save dialog on desktop Tauri, or an
 * anchor-download in the browser. Returns `true` only when written via the
 * desktop dialog (so the caller can confirm with a toast). Throws on
 * unexpected failure.
 */
export async function saveFileAttachment(
	file: FileAttachment,
): Promise<boolean> {
	if (isTauriEnv() && isMobile) {
		await saveAndOpenFile(file);
		return false;
	}
	return saveToDisk(file);
}

async function saveToDisk(
	file: FileAttachment | PhotoAttachment,
): Promise<boolean> {
	const data = await loadMediaBytes(file);
	return saveFile(
		data,
		await downloadDir().catch(() => ''),
		file.name,
		file.mime_type,
		m.saveFile(),
	);
}

/** Webview URL that the `irohblob://` URI scheme handler serves the blob's
 * bytes from. The handler reads the blob from the node's local store. */
export function blobUrl(hash: Hash): string {
	return convertFileSrc(hash, 'irohblob');
}

/**
 * Source URL for rendering a media item: the `irohblob://` URL its bytes are
 * served from. The handler reads the blob from the node's local store; the
 * webview caches the response (hashes are content-addressed, so immutable).
 */
export function mediaSrc(item: PhotoAttachment | FileAttachment): string {
	return blobUrl(item.hash);
}

/** Display size of a media item, from its stored metadata. */
export function mediaSize(item: PhotoAttachment | FileAttachment): number {
	return item.size;
}

/** Thrown when a media item's blob bytes can't be fetched — typically because
 * the blob hasn't synced to this device yet. Expected, so callers should not
 * surface it as an unexpected error. */
export class BlobLoadError extends Error {
	constructor(hash: Hash) {
		super(`failed to load blob ${hash}`);
		this.name = 'BlobLoadError';
	}
}

/** Raw bytes of a media item, fetched from the `irohblob://` scheme. */
export async function loadMediaBytes(
	item: PhotoAttachment | FileAttachment,
): Promise<Uint8Array> {
	const res = await fetch(blobUrl(item.hash));
	if (!res.ok) throw new BlobLoadError(item.hash);
	return new Uint8Array(await res.arrayBuffer());
}
