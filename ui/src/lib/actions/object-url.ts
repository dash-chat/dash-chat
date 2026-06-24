import type { Action } from 'svelte/action';

/** Either raw bytes (received message photos) or a `Blob`/`File` (draft previews). */
type ObjectUrlSource = Blob | { data: Uint8Array; mimeType: string };

/**
 * Point an `<img>` or `<audio>` element at a `blob:` URL built from its source,
 * revoking the URL when the source changes or the element is destroyed so
 * object URLs can't leak. The URL is only rebuilt when the underlying
 * bytes/blob actually change, so unrelated re-renders don't reload the media.
 */
export const objectUrl: Action<
	HTMLImageElement | HTMLAudioElement,
	ObjectUrlSource | undefined
> = (node, source) => {
	let url = '';
	let current: Blob | Uint8Array | null = null;

	function apply(s: ObjectUrlSource | undefined) {
		if (!s) return;
		const key = s instanceof Blob ? s : s.data;
		if (key === current) return;
		if (url) URL.revokeObjectURL(url);
		url = URL.createObjectURL(
			s instanceof Blob ? s : new Blob([s.data], { type: s.mimeType }),
		);
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
