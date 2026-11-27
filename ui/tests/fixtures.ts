/**
 * Throwaway fixtures for whatever just needs valid content where the content
 * doesn't matter: e2e specs and the scenes of the translation screenshots.
 * Runs in the page as well as in Node, so nothing here may use Node's APIs.
 */

/** A 1×1 PNG as a data URL, encoded as RGBA (colour type 6). Not
 *  greyscale+alpha (colour type 4): `createImageBitmap` rejects those on the
 *  Android WebView, so the app's send path can't measure them. */
export const TINY_PNG_DATA_URL =
	'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==';

/** The same 1×1 PNG as a byte array, for APIs that take raw bytes. */
export const TINY_PNG_BYTES: number[] = Array.from(
	atob(TINY_PNG_DATA_URL.slice(TINY_PNG_DATA_URL.indexOf(',') + 1)),
	char => char.charCodeAt(0),
);
