import { crc32, deflateSync } from 'node:zlib';

/**
 * Throwaway image fixtures for tests that just need a valid, decodable picture
 * (a staged attachment, an injected recent photo, …) where the pixels don't
 * matter. The `data:image/png;base64,` form is canonical; byte/array variants
 * derive from it.
 */

/** A 1×1 transparent PNG as a data URL, encoded as RGBA (colour type 6).
 *  Not greyscale+alpha (colour type 4): `createImageBitmap` rejects those on
 *  the Android WebView, so the app's send path can't measure them. */
export const TINY_PNG_DATA_URL =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNgAAIAAAUAAen63NgAAAAASUVORK5CYII=';

/** The same 1×1 PNG as a byte array, for APIs that take raw bytes. */
export const TINY_PNG_BYTES = dataUrlToBytes(TINY_PNG_DATA_URL);

/**
 * A 2×2 PNG of one solid colour, for tests that follow one specific image
 * through the app and assert on it at the far end.
 */
export const SOLID_PNG_DATA_URL =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGN4EckDRAwQCgAn6gU1/7HHIQAAAABJRU5ErkJggg==';

/** The same solid PNG as a byte array, for APIs that take raw bytes. */
export const SOLID_PNG_BYTES = dataUrlToBytes(SOLID_PNG_DATA_URL);

/** Decode a `data:...;base64,` URL into a plain byte array. */
export function dataUrlToBytes(dataUrl: string): number[] {
	const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1);
	return Array.from(Buffer.from(base64, 'base64'));
}

/**
 * A `size`×`size` opaque PNG of pseudo-random pixels. Noise doesn't compress,
 * so it stays large through the app's re-encoding: the worst case for
 * anything that carries an avatar's bytes around.
 */
export function noisePngBytes(size: number, seed: number): number[] {
	const rowLength = 1 + size * 3;
	const pixels = Buffer.alloc(rowLength * size);
	let x = seed >>> 0 || 1;
	for (let i = 0; i < pixels.length; i++) {
		x ^= x << 13;
		x >>>= 0;
		x ^= x >>> 17;
		x ^= x << 5;
		x >>>= 0;
		pixels[i] = i % rowLength === 0 ? 0 : x & 0xff;
	}
	const header = Buffer.alloc(13);
	header.writeUInt32BE(size, 0);
	header.writeUInt32BE(size, 4);
	header.set([8, 2, 0, 0, 0], 8);
	return Array.from(
		Buffer.concat([
			Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
			pngChunk('IHDR', header),
			pngChunk('IDAT', deflateSync(pixels)),
			pngChunk('IEND', Buffer.alloc(0)),
		]),
	);
}

function pngChunk(type: string, data: Buffer): Buffer {
	const typeAndData = Buffer.concat([Buffer.from(type, 'ascii'), data]);
	const length = Buffer.alloc(4);
	length.writeUInt32BE(data.length);
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(typeAndData));
	return Buffer.concat([length, typeAndData, crc]);
}
