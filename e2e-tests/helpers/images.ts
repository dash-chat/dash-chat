/**
 * Throwaway image fixtures for tests that just need a valid, decodable picture
 * (a staged attachment, an injected recent photo, …) where the pixels don't
 * matter. The `data:image/png;base64,` form is canonical; byte/array variants
 * derive from it.
 */

/** A 1×1 transparent PNG as a data URL. */
export const TINY_PNG_DATA_URL =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==';

/** The same 1×1 PNG as a byte array, for APIs that take raw bytes. */
export const TINY_PNG_BYTES = dataUrlToBytes(TINY_PNG_DATA_URL);

/** RGB of every pixel in {@link SOLID_PNG_DATA_URL}. */
export const SOLID_PNG_RGB = { r: 0xe8, g: 0x59, b: 0x0c };

/**
 * A 2×2 PNG filled with {@link SOLID_PNG_RGB}, for tests that follow one
 * specific image through the app and assert on its pixels at the far end.
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
