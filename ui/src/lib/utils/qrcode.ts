import { isTauriEnv } from '$lib/utils/environment';
import jsQR from 'jsqr';

export async function scanQrCode(): Promise<string> {
	if (!isTauriEnv()) {
		throw new Error('QR code scanning requires the Tauri desktop/mobile app');
	}
	const { Format, checkPermissions, requestPermissions, scan } = await import(
		'@tauri-apps/plugin-barcode-scanner'
	);

	await ensureCameraPermission(checkPermissions, requestPermissions);

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
async function ensureCameraPermission(
	checkPermissions: () => Promise<string>,
	requestPermissions: () => Promise<string>,
): Promise<void> {
	const state = await checkPermissions();
	if (state === 'granted') return;

	// Start the permission request (may hang due to plugin bug)
	const requestPromise = requestPermissions().catch(() => {});

	// Poll checkPermissions as a fallback — the OS grants the permission
	// even if the plugin callback never fires.
	const granted = await Promise.race([
		requestPromise.then(() => true),
		pollUntilGranted(checkPermissions),
	]);

	if (!granted) {
		throw new Error('Camera permission not granted');
	}
}

async function pollUntilGranted(
	checkPermissions: () => Promise<string>,
): Promise<boolean> {
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
					reject(new Error('No QR code found in image'));
				}
			};
			img.onerror = () => reject(new Error('Failed to load image'));
			img.src = e.target?.result as string;
		};
		reader.onerror = () => reject(new Error('Failed to read file'));
		reader.readAsDataURL(file);
	});
}
