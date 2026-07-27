import { isIos } from '$lib/utils/environment';
import {
	mdiArrowLeft,
	mdiArrowRight,
	mdiExportVariant,
	mdiShareVariant,
} from '@mdi/js';

// Wraps a path from @mdi/js into an svg, to be used inside an <sl-icon src=""></sl-icon>
export function wrapPathInSvg(path: string): string {
	return `data:image/svg+xml;utf8,${wrapPathInSvgWithoutPrefix(path)}`;
}

// Wraps a path from @mdi/js into an svg, to be used inside an <sl-icon src=""></sl-icon>
export function wrapPathInSvgWithoutPrefix(path: string): string {
	return encodeURIComponent(
		`<svg xmlns='http://www.w3.org/2000/svg' style='fill: currentColor' viewBox='0 0 24 24'><path d='${path}'></path></svg>`,
	);
}

export const mdiArrowBack =
	document.dir === 'rtl' ? mdiArrowRight : mdiArrowLeft;
export const mdiArrowNext =
	document.dir === 'rtl' ? mdiArrowLeft : mdiArrowRight;
export const mdiShare = isIos ? mdiExportVariant : mdiShareVariant;
