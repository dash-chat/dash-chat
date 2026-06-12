const COMPRESSIBLE_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp']);

const MAX_LONG_SIDE_PX = 1920;
const JPEG_QUALITY = 0.85;

/**
 * Re-encode raster images (jpeg/png/webp) to JPEG, scaled so the long side is
 * at most 1920px. Returns the original file if it's not a compressible type,
 * if the browser can't decode it, or if compression doesn't reduce the size.
 */
export async function compressImage(file: File): Promise<File> {
	if (!COMPRESSIBLE_TYPES.has(file.type)) return file;

	const bitmap = await tryCreateImageBitmap(file);
	if (!bitmap) return file;

	const { width, height } = scaledDimensions(bitmap.width, bitmap.height);
	const blob = await drawToJpegBlob(bitmap, width, height);
	bitmap.close();
	if (!blob || blob.size >= file.size) return file;

	const newName = replaceExtension(file.name, 'jpg');
	return new File([blob], newName, { type: 'image/jpeg' });
}

async function tryCreateImageBitmap(file: File): Promise<ImageBitmap | null> {
	try {
		return await createImageBitmap(file);
	} catch {
		return null;
	}
}

function scaledDimensions(
	w: number,
	h: number,
): { width: number; height: number } {
	const longSide = Math.max(w, h);
	if (longSide <= MAX_LONG_SIDE_PX) return { width: w, height: h };
	const scale = MAX_LONG_SIDE_PX / longSide;
	return { width: Math.round(w * scale), height: Math.round(h * scale) };
}

async function drawToJpegBlob(
	bitmap: ImageBitmap,
	width: number,
	height: number,
): Promise<Blob | null> {
	if (typeof OffscreenCanvas !== 'undefined') {
		const canvas = new OffscreenCanvas(width, height);
		const ctx = canvas.getContext('2d');
		if (!ctx) return null;
		ctx.drawImage(bitmap, 0, 0, width, height);
		return await canvas.convertToBlob({
			type: 'image/jpeg',
			quality: JPEG_QUALITY,
		});
	}
	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const ctx = canvas.getContext('2d');
	if (!ctx) return null;
	ctx.drawImage(bitmap, 0, 0, width, height);
	return await new Promise<Blob | null>(resolve =>
		canvas.toBlob(resolve, 'image/jpeg', JPEG_QUALITY),
	);
}

function replaceExtension(name: string, ext: string): string {
	const dot = name.lastIndexOf('.');
	return dot > 0 ? `${name.slice(0, dot)}.${ext}` : `${name}.${ext}`;
}
