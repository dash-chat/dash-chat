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

/**
 * Decode an image file and export it as an avatar-sized data URL. Rejects if
 * the file cannot be read or is not an image the webview can decode.
 */
export function fileToAvatar(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onerror = () => reject(new Error(`failed to read ${file.name}`));
		reader.onload = e => {
			const img = new Image();
			img.crossOrigin = 'anonymous';
			img.onerror = () => reject(new Error(`failed to decode ${file.name}`));
			img.onload = () => resolve(resizeAndExport(img));
			img.src = e.target?.result as string;
		};
		reader.readAsDataURL(file);
	});
}
