import { compressImage } from '$lib/utils/compress';
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
 * Draft media held in the composer before sending. Holds raw `File` refs so
 * the UI can render previews without copying bytes; `previewUrl` is an object
 * URL the caller must revoke when discarding.
 */
export type DraftMedia =
	| { kind: 'photos'; items: DraftPhoto[] }
	| { kind: 'file'; file: File };

export interface DraftPhoto {
	file: File;
	previewUrl: string;
}

export const MAX_STAGED_PHOTOS = 32;

export type IngestError = 'tooMany' | 'filesWithPhotos' | 'oneFileAtATime';

export interface IngestResult {
	media: DraftMedia | undefined;
	error?: IngestError;
}

function isVisualFile(file: File): boolean {
	return file.type.startsWith('image/') || file.type.startsWith('video/');
}

/**
 * Add `files` to the current draft following Signal's mixing rules: photos
 * and videos append (up to `MAX_STAGED_PHOTOS`, accepting a partial batch),
 * a non-visual file can only be staged alone, and nothing can be added once
 * a file is staged. On a rule violation the current draft is returned
 * unchanged alongside the error. Accepted files get fresh object URLs;
 * existing draft items keep theirs.
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
	const items = [
		...existing,
		...accepted.map(file => ({
			file,
			previewUrl: URL.createObjectURL(file),
		})),
	];
	return {
		media: { kind: 'photos', items },
		error: visual.length > room ? 'tooMany' : undefined,
	};
}

export function revokeDraft(draft: DraftMedia): void {
	if (draft.kind === 'photos') {
		for (const p of draft.items) URL.revokeObjectURL(p.previewUrl);
	}
}

/** Read a `File` as a `Uint8Array`. Raw bytes — no base64. */
async function fileToBytes(file: File): Promise<Uint8Array> {
	return new Uint8Array(await file.arrayBuffer());
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
			draft.items.map(async ({ file }) => {
				const compressed = await compressImage(file);
				return {
					data: await fileToBytes(compressed),
					name: compressed.name,
					mime_type: compressed.type || 'application/octet-stream',
				};
			}),
		);
		return { kind: 'photos', photos };
	}
	const file: FileAttachment = {
		data: await fileToBytes(draft.file),
		name: draft.file.name,
		mime_type: draft.file.type || 'application/octet-stream',
	};
	return { kind: 'file', file };
}

function totalMediaBytes(media: Media): number {
	if (media.kind === 'photos') {
		return media.photos.reduce((sum, p) => sum + byteLengthOf(p.data), 0);
	}
	return byteLengthOf(media.file.data);
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
 * Normalize bytes coming from Tauri IPC. The Rust side sends `Vec<u8>`, which
 * Tauri's default JSON serialization delivers as a plain `number[]` — not a
 * `Uint8Array`. Coerce here so downstream code can rely on `Uint8Array`.
 */
export function asUint8Array(
	data: Uint8Array | ArrayBuffer | number[],
): Uint8Array {
	if (data instanceof Uint8Array) return data;
	if (data instanceof ArrayBuffer) return new Uint8Array(data);
	return new Uint8Array(data);
}

/**
 * Wrap raw bytes into a Blob URL. Accepts either `Uint8Array` (in-process)
 * or `number[]` (fresh from Tauri JSON IPC). Caller is responsible for
 * revoking via `URL.revokeObjectURL` (Svelte: use `$effect` cleanup).
 */
export function bytesToBlobUrl(
	data: Uint8Array | number[],
	mimeType: string,
): string {
	return URL.createObjectURL(
		new Blob([asUint8Array(data)], { type: mimeType }),
	);
}

/** Size in bytes of either an in-process `Uint8Array` or an IPC number array. */
export function byteLengthOf(data: Uint8Array | number[]): number {
	return data instanceof Uint8Array ? data.byteLength : data.length;
}
