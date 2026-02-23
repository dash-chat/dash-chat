import {
	Format,
	requestPermissions,
	scan,
} from '@tauri-apps/plugin-barcode-scanner';

export async function scanQrcode(): Promise<string> {
	await requestPermissions();
	const result = await scan({ windowed: true, formats: [Format.QRCode] });
	return result.content;
}
