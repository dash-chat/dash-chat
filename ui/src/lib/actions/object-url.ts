import type { Action } from 'svelte/action';

interface ObjectUrlParams {
	data: Uint8Array;
	mimeType: string;
}

/**
 * Point an `<img>` at a `blob:` URL built from raw bytes, revoking the URL when
 * the bytes change or the element is destroyed so object URLs can't leak. The
 * URL is only rebuilt when `data` actually changes, so unrelated re-renders
 * don't reload the image.
 */
export const objectUrl: Action<HTMLImageElement, ObjectUrlParams> = (
	node,
	params,
) => {
	let url = '';
	let current: Uint8Array | null = null;

	function apply(p: ObjectUrlParams) {
		if (p.data === current) return;
		if (url) URL.revokeObjectURL(url);
		url = URL.createObjectURL(new Blob([p.data], { type: p.mimeType }));
		current = p.data;
		node.src = url;
	}

	apply(params);

	return {
		update: apply,
		destroy() {
			if (url) URL.revokeObjectURL(url);
		},
	};
};
