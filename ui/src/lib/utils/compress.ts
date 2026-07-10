import { ImageBlobReduce } from 'image-blob-reduce';

const COMPRESSIBLE_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp']);

const MAX_LONG_SIDE_PX = 1920;
const JPEG_QUALITY = 0.85;

const reduce = new ImageBlobReduce();

/**
 * Re-encode raster images (jpeg/png/webp) to JPEG, scaled so the long side is
 * at most 1920px. Returns the original file if it's not a compressible type,
 * if the browser can't decode it, or if compression doesn't reduce the size.
 */
export async function compressImage(file: File): Promise<File> {
	if (!COMPRESSIBLE_TYPES.has(file.type)) return file;

	let blob: Blob;
	try {
		const canvas = await reduce.toCanvas(file, { max: MAX_LONG_SIDE_PX });
		blob = await encodeJpegOnWhite(canvas);
	} catch {
		return file;
	}
	if (blob.size >= file.size) return file;

	const newName = replaceExtension(file.name, 'jpg');
	return new File([blob], newName, { type: 'image/jpeg' });
}

/** JPEG has no alpha channel; without a fill, transparent pixels flatten to
 * the canvas's transparent-black default. */
function encodeJpegOnWhite(
	source: HTMLCanvasElement | OffscreenCanvas,
): Promise<Blob> {
	const flattened = document.createElement('canvas');
	flattened.width = source.width;
	flattened.height = source.height;
	const ctx = flattened.getContext('2d');
	if (!ctx) return Promise.reject(new Error('no 2d context'));
	ctx.fillStyle = 'white';
	ctx.fillRect(0, 0, flattened.width, flattened.height);
	ctx.drawImage(source, 0, 0);
	return new Promise((resolve, reject) =>
		flattened.toBlob(
			blob => (blob ? resolve(blob) : reject(new Error('toBlob failed'))),
			'image/jpeg',
			JPEG_QUALITY,
		),
	);
}

function replaceExtension(name: string, ext: string): string {
	const dot = name.lastIndexOf('.');
	return dot > 0 ? `${name.slice(0, dot)}.${ext}` : `${name}.${ext}`;
}
