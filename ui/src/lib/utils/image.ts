import { ImageBlobReduce } from 'image-blob-reduce';

const AVATAR_MAX_PX = 300;

const reduce = new ImageBlobReduce();

/**
 * Decode an image file and export it as an avatar-sized data URL. Rejects if
 * the file cannot be read or is not an image the webview can decode.
 */
export async function fileToAvatar(file: File): Promise<string> {
	const canvas = await reduce
		.toCanvas(file, { max: AVATAR_MAX_PX })
		.catch(() => {
			throw new Error(`failed to decode ${file.name}`);
		});
	if (canvas instanceof HTMLCanvasElement) return canvas.toDataURL();

	const out = document.createElement('canvas');
	out.width = canvas.width;
	out.height = canvas.height;
	const ctx = out.getContext('2d');
	if (!ctx) throw new Error(`failed to decode ${file.name}`);
	ctx.drawImage(canvas, 0, 0);
	return out.toDataURL();
}
