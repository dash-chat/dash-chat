import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import { shareFile } from '@choochmeque/tauri-plugin-sharekit-api';
import { appCacheDir, join } from '@tauri-apps/api/path';

/**
 * Gets the QR code as a drawable image source from the wa-qr-code shadow DOM.
 * Tries canvas first, falls back to rendering the SVG onto a canvas.
 */
async function getQrSource(
	qrCard: HTMLElement,
): Promise<{ source: CanvasImageSource; size: number } | undefined> {
	const waQr = qrCard.querySelector('wa-qr-code');
	if (!waQr?.shadowRoot) return;

	// Try canvas first (desktop)
	const srcCanvas = waQr.shadowRoot.querySelector('canvas') as HTMLCanvasElement | null;
	if (srcCanvas && srcCanvas.width > 0) {
		return { source: srcCanvas, size: srcCanvas.width };
	}

	// Fall back to SVG (mobile)
	const svgEl = waQr.shadowRoot.querySelector('svg') as SVGElement | null;
	if (!svgEl) return;

	const svgData = new XMLSerializer().serializeToString(svgEl);
	const img = new Image();
	await new Promise<void>((resolve, reject) => {
		img.onload = () => resolve();
		img.onerror = () => reject(new Error('Failed to load SVG'));
		img.src = 'data:image/svg+xml;base64,' + btoa(svgData);
	});

	return { source: img, size: img.width };
}

/**
 * Renders the QR code card to PNG bytes.
 */
async function renderQrImage(qrColor: string): Promise<Uint8Array | undefined> {
	const qrCard = document.querySelector('.qr-card') as HTMLElement | null;
	if (!qrCard) return;

	const qrSource = await getQrSource(qrCard);
	if (!qrSource) return;

	const padding = 32;
	const qrSize = qrSource.size;
	const totalSize = qrSize + padding * 2;

	const canvas = document.createElement('canvas');
	canvas.width = totalSize;
	canvas.height = totalSize;
	const ctx = canvas.getContext('2d')!;

	// Draw colored background with rounded corners
	ctx.fillStyle = qrColor;
	drawRoundedRect(ctx, 0, 0, totalSize, totalSize, 24);
	ctx.fill();

	// Draw white inner area with padding
	const innerPad = 16;
	ctx.fillStyle = 'white';
	drawRoundedRect(
		ctx,
		padding - innerPad,
		padding - innerPad,
		qrSize + innerPad * 2,
		qrSize + innerPad * 2,
		12,
	);
	ctx.fill();

	// Draw QR code
	ctx.drawImage(qrSource.source, padding, padding, qrSize, qrSize);

	// Get PNG bytes
	const blob = await new Promise<Blob>((resolve) => {
		canvas.toBlob((b) => resolve(b!), 'image/png');
	});
	return new Uint8Array(await blob.arrayBuffer());
}

/**
 * Renders the QR code card to a PNG image and saves it via a native save dialog.
 */
export async function saveQrCode(qrColor: string): Promise<void> {
	const bytes = await renderQrImage(qrColor);
	if (!bytes) return;

	const path = await save({
		title: 'Save QR Code',
		defaultPath: 'dashchat-qr-code.png',
		filters: [{ name: 'PNG Image', extensions: ['png'] }],
	});

	if (path) {
		await writeFile(path, bytes);
	}
}

/**
 * Renders the QR code card to a PNG image and shares it via native share sheet.
 */
export async function shareQrCode(qrColor: string): Promise<void> {
	const bytes = await renderQrImage(qrColor);
	if (!bytes) return;

	const cacheDir = await appCacheDir();
	const path = await join(cacheDir, 'dashchat-qr-code.png');
	await writeFile(path, bytes);

	await shareFile(`file://${path}`, {
		mimeType: 'image/png',
		title: 'dashchat-qr-code.png',
	});
}

function drawRoundedRect(
	ctx: CanvasRenderingContext2D,
	x: number,
	y: number,
	w: number,
	h: number,
	r: number,
): void {
	ctx.beginPath();
	ctx.moveTo(x + r, y);
	ctx.lineTo(x + w - r, y);
	ctx.quadraticCurveTo(x + w, y, x + w, y + r);
	ctx.lineTo(x + w, y + h - r);
	ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
	ctx.lineTo(x + r, y + h);
	ctx.quadraticCurveTo(x, y + h, x, y + h - r);
	ctx.lineTo(x, y + r);
	ctx.quadraticCurveTo(x, y, x + r, y);
	ctx.closePath();
}
