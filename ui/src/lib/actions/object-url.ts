import type { Action } from 'svelte/action';

/**
 * Either raw bytes (received message photos), a `Blob`/`File` (draft previews),
 * or an already-usable URL string (e.g. a `data:` URL) that is used as-is.
 */
type ObjectUrlSource = Blob | { data: Uint8Array; mimeType: string } | string;

/**
 * Point an `<img>` at a `blob:` URL built from its source, revoking the URL
 * when the source changes or the element is destroyed so object URLs can't
 * leak. The URL is only rebuilt when the underlying bytes/blob actually change,
 * so unrelated re-renders don't reload the image. A plain string source is set
 * directly with no object URL to manage.
 */
export const objectUrl: Action<
	HTMLImageElement | HTMLAudioElement,
	ObjectUrlSource | undefined
> = (node, source) => {
	let url = '';
	let current: Blob | Uint8Array | null = null;

	function apply(s: ObjectUrlSource | undefined) {
		if (!s) return;
		if (typeof s === 'string') {
			if (url) URL.revokeObjectURL(url);
			url = '';
			current = null;
			node.src = s;
			return;
		}
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
