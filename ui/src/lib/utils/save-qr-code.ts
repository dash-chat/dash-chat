import { shareFile } from '@choochmeque/tauri-plugin-sharekit-api';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { save } from '@tauri-apps/plugin-dialog';
import { mkdir, writeFile } from '@tauri-apps/plugin-fs';
import QrCreator from 'qr-creator';
import { m } from '$lib/paraglide/messages.js';

const FONT_FAMILY = "-apple-system, 'Segoe UI', Roboto, sans-serif";
const HEX_COLOR_RE = /^#[0-9a-fA-F]{6}$/;

function sanitizeHexColor(color: string, fallback = '#007aff'): string {
	return HEX_COLOR_RE.test(color) ? color : fallback;
}

function roundRect(
	ctx: CanvasRenderingContext2D,
	x: number,
	y: number,
	w: number,
	h: number,
	r: number,
) {
	ctx.beginPath();
	ctx.roundRect(x, y, w, h, r);
	ctx.fill();
}

/**
 * Renders the QR share image directly to a canvas and returns PNG bytes.
 */
function renderQrImage(
	code: string,
	rawColor: string,
	name: string,
): Promise<Uint8Array> {
	const qrColor = sanitizeHexColor(rawColor);
	// Render QR to a temporary canvas
	const qrSize = 480;
	const isWhite = qrColor === '#ffffff';
	const qrFill = isWhite ? '#000000' : qrColor;
	const qrCanvas = document.createElement('canvas');
	QrCreator.render(
		{
			text: code,
			size: qrSize,
			fill: qrFill,
			background: null,
			ecLevel: 'L',
			radius: 0.5,
		},
		qrCanvas,
	);

	// Layout dimensions
	const imgWidth = 840;
	const cardMargin = 90;
	const cardWidth = imgWidth - cardMargin * 2;
	const cardRadius = 72;
	const qrDisplaySize = 480;
	const qrWhitePad = 36;
	const whiteAreaSize = qrDisplaySize + qrWhitePad * 2;
	const qrWhiteRadius = 36;

	const cardTop = 90;
	const qrPadding = 60;
	const qrWhiteTop = cardTop + qrPadding;
	const qrWhiteLeft = (imgWidth - whiteAreaSize) / 2;
	const qrTop = qrWhiteTop + qrWhitePad;
	const qrLeft = qrWhiteLeft + qrWhitePad;

	const nameFontSize = 48;
	const bottomMargin = 120;
	const qrWhiteBottom = qrWhiteTop + whiteAreaSize;
	const cardBottom = qrWhiteBottom + bottomMargin;
	const cardHeight = cardBottom - cardTop;
	const nameCenterY = qrWhiteBottom + bottomMargin / 2;

	const subtitleTop = cardBottom + 72;
	const subtitleFontSize = 33;
	const subtitle = m.shareQrCodeSubtitle();
	const totalHeight = subtitleTop + subtitleFontSize + 90;

	// Draw everything directly to canvas
	const canvas = document.createElement('canvas');
	canvas.width = imgWidth;
	canvas.height = totalHeight;
	const ctx = canvas.getContext('2d')!;

	// Background
	ctx.fillStyle = '#e8e4f0';
	ctx.fillRect(0, 0, imgWidth, totalHeight);

	// Card
	ctx.fillStyle = qrColor;
	roundRect(ctx, cardMargin, cardTop, cardWidth, cardHeight, cardRadius);

	// White area behind QR
	ctx.fillStyle = 'white';
	roundRect(ctx, qrWhiteLeft, qrWhiteTop, whiteAreaSize, whiteAreaSize, qrWhiteRadius);

	// QR code
	ctx.drawImage(qrCanvas, qrLeft, qrTop, qrDisplaySize, qrDisplaySize);

	// Name text
	ctx.fillStyle = isWhite ? 'black' : 'white';
	ctx.font = `bold ${nameFontSize}px ${FONT_FAMILY}`;
	ctx.textAlign = 'center';
	ctx.textBaseline = 'middle';
	ctx.fillText(name, imgWidth / 2, nameCenterY);

	// Subtitle text
	ctx.fillStyle = '#555555';
	ctx.font = `${subtitleFontSize}px ${FONT_FAMILY}`;
	ctx.textBaseline = 'alphabetic';
	ctx.fillText(subtitle, imgWidth / 2, subtitleTop + subtitleFontSize);

	return new Promise((resolve, reject) => {
		canvas.toBlob(
			(blob) => {
				if (!blob) {
					reject(new Error('Failed to render QR image'));
					return;
				}
				blob.arrayBuffer().then(
					(buf) => resolve(new Uint8Array(buf)),
					reject,
				);
			},
			'image/png',
		);
	});
}

/**
 * Renders the QR code to a PNG image and saves it via a native save dialog.
 */
export async function saveQrCode(
	code: string,
	qrColor: string,
	name: string,
): Promise<void> {
	const bytes = await renderQrImage(code, qrColor, name);

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
 * Renders the QR code to a PNG image and shares it via native share sheet.
 */
export async function shareQrCode(
	code: string,
	qrColor: string,
	name: string,
): Promise<void> {
	const bytes = await renderQrImage(code, qrColor, name);

	const cacheDir = await appCacheDir();
	const shareDir = await join(cacheDir, 'share');
	await mkdir(shareDir, { recursive: true });
	const path = await join(shareDir, 'dashchat-qr-code.png');
	await writeFile(path, bytes);

	await shareFile(`file://${path}`, {
		mimeType: 'image/png',
		title: 'dashchat-qr-code.png',
	});
}
