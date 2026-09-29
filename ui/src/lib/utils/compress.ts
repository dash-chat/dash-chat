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
	if (!ctx) throw new Error('no 2d context');
	ctx.fillStyle = 'white';
	ctx.fillRect(0, 0, flattened.width, flattened.height);
	ctx.drawImage(source, 0, 0);
	// Not toBlob: Android WebView only runs async canvas encodes while the page
	// is producing frames, so on a still page each one waits ~4s for a timeout.
	return dataUrlToBlob(flattened.toDataURL('image/jpeg', JPEG_QUALITY));
}

async function dataUrlToBlob(dataUrl: string): Promise<Blob> {
	const blob = await (await fetch(dataUrl)).blob();
	if (blob.type !== 'image/jpeg') throw new Error('JPEG encode failed');
	return blob;
}

function replaceExtension(name: string, ext: string): string {
	const dot = name.lastIndexOf('.');
	return dot > 0 ? `${name.slice(0, dot)}.${ext}` : `${name}.${ext}`;
}
