import QrCreator from 'qr-creator';

import { S } from '../selectors';

/**
 * Generate a QR code image for the given contact code and inject it into the
 * hidden file input, simulating the user selecting an image via the upload button.
 */
export async function uploadQrCodeImage(code: string): Promise<true> {
	const canvas = document.createElement('canvas');
	QrCreator.render(
		{
			text: code,
			size: 256,
			fill: '#000000',
			background: '#ffffff',
			ecLevel: 'L',
			radius: 0,
		},
		canvas,
	);

	const blob = await new Promise<Blob>((resolve, reject) => {
		canvas.toBlob(
			b => (b ? resolve(b) : reject(new Error('canvas.toBlob failed'))),
			'image/png',
		);
	});

	const file = new File([blob], 'qr.png', { type: 'image/png' });
	const dt = new DataTransfer();
	dt.items.add(file);

	const input = document.querySelector(
		`${S.addContact.fileInput}`,
	) as HTMLInputElement | null;
	if (!input) throw new Error('QR file input not found');

	input.files = dt.files;
	input.dispatchEvent(new Event('change', { bubbles: true }));

	return true;
}

/** Inject a blank (no QR code) PNG image into the hidden file input. */
export async function uploadEmptyImage(): Promise<true> {
	const canvas = document.createElement('canvas');
	canvas.width = 64;
	canvas.height = 64;
	canvas.getContext('2d')!.fillStyle = '#ffffff';
	canvas.getContext('2d')!.fillRect(0, 0, 64, 64);

	const blob = await new Promise<Blob>((resolve, reject) => {
		canvas.toBlob(
			b => (b ? resolve(b) : reject(new Error('canvas.toBlob failed'))),
			'image/png',
		);
	});

	const file = new File([blob], 'blank.png', { type: 'image/png' });
	const dt = new DataTransfer();
	dt.items.add(file);

	const input = document.querySelector(
		`${S.addContact.fileInput}`,
	) as HTMLInputElement | null;
	if (!input) throw new Error('QR file input not found');

	input.files = dt.files;
	input.dispatchEvent(new Event('change', { bubbles: true }));

	return true;
}
