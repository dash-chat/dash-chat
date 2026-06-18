import type { Action } from 'svelte/action';

/** Either raw bytes (received message photos) or a `Blob`/`File` (draft previews). */
type ObjectUrlSource = Blob | { data: Uint8Array; mimeType: string };

function sourceKey(source: ObjectUrlSource): Blob | Uint8Array {
	return source instanceof Blob ? source : source.data;
}

function toBlob(source: ObjectUrlSource): Blob {
	return source instanceof Blob
		? source
		: new Blob([source.data], { type: source.mimeType });
}

/**
 * Point an `<img>` at a `blob:` URL built from its source, revoking the URL
 * when the source changes or the element is destroyed so object URLs can't
 * leak. The URL is only rebuilt when the underlying bytes/blob actually change,
 * so unrelated re-renders don't reload the image.
 */
export const objectUrl: Action<HTMLImageElement, ObjectUrlSource> = (
	node,
	source,
) => {
	let url = '';
	let current: Blob | Uint8Array | null = null;

	function apply(s: ObjectUrlSource) {
		const key = sourceKey(s);
		if (key === current) return;
		if (url) URL.revokeObjectURL(url);
		url = URL.createObjectURL(toBlob(s));
		current = key;
		node.src = url;
	}

	apply(source);

	return {
		update: apply,
		destroy() {
			if (url) URL.revokeObjectURL(url);
		},
	};
};
