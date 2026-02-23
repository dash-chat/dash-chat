import { shareFile } from '@choochmeque/tauri-plugin-sharekit-api';
import { appCacheDir, join } from '@tauri-apps/api/path';
import { save } from '@tauri-apps/plugin-dialog';
import { mkdir, writeFile } from '@tauri-apps/plugin-fs';
import QrCreator from 'qr-creator';
import { m } from '$lib/paraglide/messages.js';

/**
 * Renders the QR code pattern to an SVG string using qr-creator's canvas output,
 * then builds a full share image as SVG with card, name, and subtitle.
 */
function buildShareSvg(
	code: string,
	qrColor: string,
	name: string,
): string {
	// Render QR to a temporary canvas to extract the pattern
	const qrSize = 480;
	const qrCanvas = document.createElement('canvas');
	QrCreator.render(
		{
			text: code,
			size: qrSize,
			fill: qrColor,
			background: null,
			ecLevel: 'L',
			radius: 0.5,
		},
		qrCanvas,
	);
	const qrDataUrl = qrCanvas.toDataURL('image/png');

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

	const nameTop = qrWhiteTop + whiteAreaSize + 48;
	const nameFontSize = 48;
	const cardBottom = nameTop + nameFontSize + 48;
	const cardHeight = cardBottom - cardTop;

	const subtitleTop = cardBottom + 72;
	const subtitleFontSize = 33;
	const subtitle = m.shareQrCodeSubtitle();
	const totalHeight = subtitleTop + subtitleFontSize + 90;

	return `<svg xmlns="http://www.w3.org/2000/svg" width="${imgWidth}" height="${totalHeight}">
	<rect width="${imgWidth}" height="${totalHeight}" fill="#e8e4f0" rx="0"/>
	<rect x="${cardMargin}" y="${cardTop}" width="${cardWidth}" height="${cardHeight}" rx="${cardRadius}" fill="${qrColor}"/>
	<rect x="${qrWhiteLeft}" y="${qrWhiteTop}" width="${whiteAreaSize}" height="${whiteAreaSize}" rx="${qrWhiteRadius}" fill="white"/>
	<image href="${qrDataUrl}" x="${qrLeft}" y="${qrTop}" width="${qrDisplaySize}" height="${qrDisplaySize}"/>
	<text x="${imgWidth / 2}" y="${nameTop + nameFontSize}" text-anchor="middle" fill="white" font-family="-apple-system, 'Segoe UI', Roboto, sans-serif" font-size="${nameFontSize}" font-weight="bold">${escapeXml(name)}</text>
	<text x="${imgWidth / 2}" y="${subtitleTop + subtitleFontSize}" text-anchor="middle" fill="#555555" font-family="-apple-system, 'Segoe UI', Roboto, sans-serif" font-size="${subtitleFontSize}">${escapeXml(subtitle)}</text>
</svg>`;
}

function escapeXml(s: string): string {
	return s
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

/**
 * Converts the SVG share image to PNG bytes via canvas.
 */
async function renderQrImage(
	code: string,
	qrColor: string,
	name: string,
): Promise<Uint8Array | undefined> {
	const svg = buildShareSvg(code, qrColor, name);
	const blob = new Blob([svg], { type: 'image/svg+xml' });
	const url = URL.createObjectURL(blob);

	const img = new Image();
	try {
		await new Promise<void>((resolve, reject) => {
			img.onload = () => resolve();
			img.onerror = () => reject(new Error('Failed to load SVG'));
			img.src = url;
		});
	} finally {
		URL.revokeObjectURL(url);
	}

	const canvas = document.createElement('canvas');
	canvas.width = img.naturalWidth;
	canvas.height = img.naturalHeight;
	const ctx = canvas.getContext('2d')!;
	ctx.drawImage(img, 0, 0);

	const dataUrl = canvas.toDataURL('image/png');
	const base64 = dataUrl.split(',')[1];
	const raw = atob(base64);
	const bytes = new Uint8Array(raw.length);
	for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
	return bytes;
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
 * Renders the QR code to a PNG image and shares it via native share sheet.
 */
export async function shareQrCode(
	code: string,
	qrColor: string,
	name: string,
): Promise<void> {
	const bytes = await renderQrImage(code, qrColor, name);
	if (!bytes) return;

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
