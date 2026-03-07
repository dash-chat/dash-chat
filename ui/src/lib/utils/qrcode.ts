import jsQR from 'jsqr';
import { isTauriEnv } from '$lib/utils/environment';

export async function scanQrcode(): Promise<string> {
	if (!isTauriEnv()) {
		throw new Error('QR code scanning requires the Tauri desktop/mobile app');
	}
	const { Format, requestPermissions, scan } = await import('@tauri-apps/plugin-barcode-scanner');
	await requestPermissions();
	const result = await scan({ windowed: true, formats: [Format.QRCode] });
	return result.content;
}

export function scanQrFromImage(file: File): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onload = (e) => {
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
