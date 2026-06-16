export interface ImageDimensions {
	width: number;
	height: number;
}

/**
 * Signal's timeline sizing for a lone image: render width clamped to
 * [200, 300]px, height following the natural aspect ratio clamped to
 * [50, 450]px.
 */
export function getTimelineImageDimensions(
	naturalWidth: number,
	naturalHeight: number,
): ImageDimensions {
	if (naturalWidth <= 0 || naturalHeight <= 0) {
		return { width: 200, height: 50 };
	}
	const width = Math.min(300, Math.max(200, naturalWidth));
	const height = Math.min(
		450,
		Math.max(50, Math.round(width * (naturalHeight / naturalWidth))),
	);
	return { width, height };
}
