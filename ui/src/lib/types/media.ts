import type { FileAttachment, Media, Photo } from 'dash-chat-stores';

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

export function makeDraftPhotos(files: FileList | File[]): DraftMedia {
	const arr = Array.from(files);
	return {
		kind: 'photos',
		items: arr.map(file => ({ file, previewUrl: URL.createObjectURL(file) })),
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

/** Convert composer-side draft to the wire-format `Media` for sending. */
export async function draftToMedia(draft: DraftMedia): Promise<Media> {
	if (draft.kind === 'photos') {
		const photos: Photo[] = await Promise.all(
			draft.items.map(async ({ file }) => ({
				data: await fileToBytes(file),
				name: file.name,
				mime_type: file.type || 'application/octet-stream',
			})),
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
