// === UI-side types (contain File references for display/upload) ===

export interface PhotoItem {
	/** Data URL for thumbnail display */
	dataUrl: string;
	/** Original file for sending */
	file: File;
}

export interface PhotosMedia {
	kind: 'photos';
	photos: PhotoItem[];
}

export interface FileMedia {
	kind: 'file';
	file: File;
	name: string;
	size: number;
}

export interface AudioMedia {
	kind: 'audio';
	/** Base64 data URL of the recorded audio */
	dataUrl: string;
	mimeType: string;
	/** Duration in milliseconds */
	durationMs: number;
	/** Size in bytes */
	size: number;
}

/** UI-side media attachment (contains File refs for display and upload) */
export type Media = PhotosMedia | FileMedia | AudioMedia;

// === Wire types (serializable, sent to backend as part of MessageContent) ===

export interface PhotoAttachment {
	/** Base64 data URL */
	data: string;
	name: string;
	mime_type: string;
	size: number;
}

export interface FileAttachment {
	/** Base64 data URL */
	data: string;
	name: string;
	mime_type: string;
	size: number;
}

export interface AudioAttachment {
	data: string;
	mime_type: string;
	duration_ms: number;
	size: number;
}

export type MediaAttachment =
	| { kind: 'photos'; photos: PhotoAttachment[] }
	| { kind: 'file'; file: FileAttachment }
	| { kind: 'audio'; audio: AudioAttachment };

/** Convert UI Media to wire-format MediaAttachment */
export async function mediaToAttachment(
	media: Media,
): Promise<MediaAttachment> {
	if (media.kind === 'photos') {
		const photos: PhotoAttachment[] = await Promise.all(
			media.photos.map(async (photo) => ({
				data: photo.dataUrl,
				name: photo.file.name,
				mime_type: photo.file.type,
				size: photo.file.size,
			})),
		);
		return { kind: 'photos', photos };
	} else if (media.kind === 'file') {
		const data = await fileToDataUrl(media.file);
		return {
			kind: 'file',
			file: {
				data,
				name: media.name,
				mime_type: media.file.type,
				size: media.size,
			},
		};
	} else {
		return {
			kind: 'audio',
			audio: {
				data: media.dataUrl,
				mime_type: media.mimeType,
				duration_ms: media.durationMs,
				size: media.size,
			},
		};
	}
}

export function formatFileSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	if (bytes < 1024 * 1024 * 1024)
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function fileToDataUrl(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = () => resolve(reader.result as string);
		reader.onerror = reject;
		reader.readAsDataURL(file);
	});
}
