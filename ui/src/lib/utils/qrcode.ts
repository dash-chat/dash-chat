import { isTauriEnv } from '$lib/utils/environment';
import {
	Format,
	checkPermissions,
	requestPermissions,
	scan,
} from '@tauri-apps/plugin-barcode-scanner';
import jsQR from 'jsqr';

import { cssColor } from './colors';

/** The brand primary colour — the default QR colour, defined once in `app.css`. */
export function defaultQrColor(): string {
	return cssColor('--color-brand-primary');
}

export type ScanQrFromImageErrorKind =
	| 'NoQrCodeFound'
	| 'LoadImageFailed'
	| 'ReadFileFailed';

export class ScanQrFromImageError extends Error {
	constructor(
		public readonly kind: ScanQrFromImageErrorKind,
		cause?: unknown,
	) {
		super(kind, { cause });
		this.name = 'ScanQrFromImageError';
	}
}

export function isScanQrFromImageError(
	error: unknown,
): error is ScanQrFromImageError {
	return error instanceof ScanQrFromImageError;
}

export async function scanQrCode(): Promise<string> {
	if (!isTauriEnv()) {
		throw new Error('QR code scanning requires the Tauri desktop/mobile app');
	}
	await ensureCameraPermission();

	const result = await scan({ windowed: true, formats: [Format.QRCode] });
	return result.content;
}

/**
 * The Tauri barcode scanner plugin's `requestPermissions()` sometimes hangs
 * on Android: the permission dialog shows and the user grants it, but the
 * plugin's internal callback never fires, so the JS promise never resolves.
 *
 * Workaround: check first with `checkPermissions()` (which always resolves).
 * If not granted, race `requestPermissions()` against a polling loop that
 * calls `checkPermissions()` until the permission is granted.
 */
async function ensureCameraPermission(): Promise<void> {
	const state = await checkPermissions();
	if (state === 'granted') return;

	// Start the permission request (may hang due to plugin bug)
	const requestPromise = requestPermissions().catch(() => 'denied' as string);

	// Poll checkPermissions as a fallback — the OS grants the permission
	// even if the plugin callback never fires.
	const granted = await Promise.race([
		requestPromise.then(state => state === 'granted'),
		pollUntilGranted(),
	]);

	if (!granted) {
		throw new Error('Camera permission not granted');
	}
}

async function pollUntilGranted(): Promise<boolean> {
	const maxWaitMs = 30_000;
	const intervalMs = 500;
	const start = Date.now();

	while (Date.now() - start < maxWaitMs) {
		await new Promise(r => setTimeout(r, intervalMs));
		const state = await checkPermissions();
		if (state === 'granted') return true;
	}
	return false;
}

export function scanQrFromImage(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = e => {
			const img = new Image();
			img.onload = () => {
				const maxDim = 2048;
				let { width, height } = img;
				if (width > maxDim || height > maxDim) {
					const scale = maxDim / Math.max(width, height);
					width = Math.round(width * scale);
					height = Math.round(height * scale);
				}
				const canvas = document.createElement('canvas');
				canvas.width = width;
				canvas.height = height;
				const ctx = canvas.getContext('2d')!;
				ctx.drawImage(img, 0, 0, width, height);
				const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
				const code = jsQR(imageData.data, imageData.width, imageData.height);
				if (code) {
					resolve(code.data);
				} else {
					reject(new ScanQrFromImageError('NoQrCodeFound'));
				}
			};
			img.onerror = event =>
				reject(new ScanQrFromImageError('LoadImageFailed', event));
			img.src = e.target?.result as string;
		};
		reader.onerror = event =>
			reject(new ScanQrFromImageError('ReadFileFailed', event));
		reader.readAsDataURL(file);
	});
}
