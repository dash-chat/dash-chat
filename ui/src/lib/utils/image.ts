export function resizeAndExport(
	img: HTMLImageElement,
	maxWidth: number = 300,
	maxHeight: number = 300,
) {
	let width = img.width;
	let height = img.height;

	// Change the resizing logic
	if (width > height) {
		if (width > maxWidth) {
			height = height * (maxWidth / width);
			width = maxWidth;
		}
	} else {
		if (height > maxHeight) {
			width = width * (maxHeight / height);
			height = maxHeight;
		}
	}

	const canvas = document.createElement('canvas');
	canvas.width = width;
	canvas.height = height;
	const ctx = canvas.getContext('2d') as CanvasRenderingContext2D;
	ctx.drawImage(img, 0, 0, width, height);

	// return the .toDataURL of the temp canvas
	return canvas.toDataURL();
}
