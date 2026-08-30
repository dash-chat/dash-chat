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
