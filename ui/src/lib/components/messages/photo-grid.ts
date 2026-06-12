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

export interface GridConfig {
	visibleCells: number;
	/** CSS aspect-ratio of the whole grid envelope. */
	aspectRatio: string;
}

/**
 * Signal's multi-photo grid shapes at a 300px-wide envelope:
 * 2 → 300×150, 3 → 300×200 (one 200² + two 100²), 4 → 2×2 of 150²,
 * 5+ → 300×250 (two 150² over three 100²) with a +N scrim on the 5th cell.
 */
export function gridConfig(count: number): GridConfig {
	if (count <= 2) return { visibleCells: count, aspectRatio: '2 / 1' };
	if (count === 3) return { visibleCells: 3, aspectRatio: '3 / 2' };
	if (count === 4) return { visibleCells: 4, aspectRatio: '1 / 1' };
	return { visibleCells: 5, aspectRatio: '6 / 5' };
}
