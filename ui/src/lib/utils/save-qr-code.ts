import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import { shareFile } from '@choochmeque/tauri-plugin-sharekit-api';
import { tempDir, join } from '@tauri-apps/api/path';

/**
 * Renders the QR code card to PNG bytes.
 */
async function renderQrImage(qrColor: string): Promise<Uint8Array | undefined> {
	const qrCard = document.querySelector('.qr-card') as HTMLElement | null;
	if (!qrCard) return;

	const srcCanvas = qrCard
		.querySelector('wa-qr-code')
		?.shadowRoot?.querySelector('canvas') as HTMLCanvasElement | null;
	if (!srcCanvas) return;

	const padding = 32;
	const qrSize = srcCanvas.width;
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

	// Draw QR code canvas
	ctx.drawImage(srcCanvas, padding, padding);

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

	const tmp = await tempDir();
	const path = await join(tmp, 'dashchat-qr-code.png');
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
