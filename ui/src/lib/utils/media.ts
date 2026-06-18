import { m } from '$lib/paraglide/messages.js';
import { compressImage } from '$lib/utils/compress';
import { isMobile, isTauriEnv } from '$lib/utils/environment';
import { saveFile, shareFile } from '$lib/utils/files';
import { downloadDir } from '@tauri-apps/api/path';
import type { FileAttachment, Media, Photo } from 'dash-chat-stores';

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
	| { kind: 'file'; file: File };

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
export async function draftToMedia(draft: DraftMedia): Promise<Media> {
	const media = await buildMedia(draft);
	const total = totalMediaBytes(media);
	if (total > MAX_MESSAGE_BYTES) {
		throw new AttachmentTooLargeError(total);
	}
	return media;
}

async function buildMedia(draft: DraftMedia): Promise<Media> {
	if (draft.kind === 'photos') {
		const photos: Photo[] = await Promise.all(
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
	const file: FileAttachment = {
		data: new Uint8Array(await draft.file.arrayBuffer()),
		name: draft.file.name,
		mime_type: draft.file.type || 'application/octet-stream',
	};
	return { kind: 'file', file };
}

function totalMediaBytes(media: Media): number {
	if (media.kind === 'photos') {
		return media.photos.reduce((sum, p) => sum + p.data.byteLength, 0);
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
 * Save an attachment: native save dialog on desktop Tauri, system share sheet
 * on mobile, anchor-download fallback in the browser. Returns `true` when the
 * file was written to disk via the desktop dialog (so the caller can confirm
 * with a toast), and `false` otherwise. Throws on unexpected failure.
 */
export async function saveAttachment(
	file: FileAttachment | Photo,
): Promise<boolean> {
	if (isTauriEnv() && isMobile) {
		await shareFile(file.data, file.name, file.mime_type);
		return false;
	}
	return saveFile(
		file.data,
		await downloadDir().catch(() => ''),
		file.name,
		file.mime_type,
		m.saveFile(),
	);
}
