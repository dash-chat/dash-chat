import type { ImageBlobReduce } from 'image-blob-reduce';

const AVATAR_MAX_PX = 300;
// A profile op must fit the LAN router's 64 KiB frame together with its header
// and the other profile fields, or the router never carries it.
const AVATAR_MAX_DATA_URL_LENGTH = 56_000;
const AVATAR_JPEG_QUALITIES_BEST_FIRST = [0.8, 0.6, 0.4, 0.2];

let reducer: Promise<ImageBlobReduce> | undefined;

/** Loaded on first use: pica is too heavy to evaluate at app startup. */
export function imageReducer(): Promise<ImageBlobReduce> {
	if (reducer === undefined) {
		reducer = import('image-blob-reduce').then(
			({ ImageBlobReduce }) => new ImageBlobReduce(),
		);
	}
	return reducer;
}

function jpegWithinAvatarBudget(canvas: HTMLCanvasElement): string {
	let dataUrl = '';
	for (const quality of AVATAR_JPEG_QUALITIES_BEST_FIRST) {
		dataUrl = canvas.toDataURL('image/jpeg', quality);
		if (dataUrl.length <= AVATAR_MAX_DATA_URL_LENGTH) break;
	}
	return dataUrl;
}

/**
 * Decode an image file and export it as an avatar-sized data URL. Rejects if
 * the file cannot be read or is not an image the webview can decode.
 */
export async function fileToAvatar(file: File): Promise<string> {
	const reduce = await imageReducer();
	const canvas = await reduce
		.toCanvas(file, { max: AVATAR_MAX_PX })
		.catch(() => {
			throw new Error(`failed to decode ${file.name}`);
		});
	if (canvas instanceof HTMLCanvasElement)
		return jpegWithinAvatarBudget(canvas);

	const out = document.createElement('canvas');
	out.width = canvas.width;
	out.height = canvas.height;
	const ctx = out.getContext('2d');
	if (!ctx) throw new Error(`failed to decode ${file.name}`);
	ctx.drawImage(canvas, 0, 0);
	return jpegWithinAvatarBudget(out);
}
