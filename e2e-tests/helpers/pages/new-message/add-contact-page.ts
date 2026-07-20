import QRCode from 'qrcode';

import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

const FILE_INPUT_TESTID = 'add-contact-file-input';

export class AddContactPage extends TestHelper {
	back = this.el(tid('add-contact-back'));
	codeTab = this.el(tid('add-contact-link-tab'));
	scanTab = this.el(tid('add-contact-scan-tab'));
	qrCode = this.el('wa-qr-code');
	copyButton = this.el(tid('add-contact-copy-btn'));
	codeInput = this.el(tid('add-contact-link-input'));
	shareButton = this.el(tid('add-contact-share-btn'));
	saveButton = this.el(tid('add-contact-save-btn'));
	uploadButton = this.el(tid('add-contact-upload-btn'));
	selectImageButton = this.el(tid('add-contact-select-image-btn'));
	fileInput = this.el(tid(FILE_INPUT_TESTID));
	colorButton = this.el(tid('add-contact-color-btn'));

	async ready() {
		await this.codeInput.waitForExist();
	}

	/** Read the contact link from the QR element. */
	async getAddContactLink(): Promise<string> {
		await this.qrCode.waitForExist();
		const link = (await this.qrCode.getProperty('value')) as string | null;
		if (!link) throw new Error('add contact link missing on QR element');
		return link;
	}

	async enterAddContactLink(link: string) {
		await this.typeInto(`${tid('add-contact-link-input')} input`, link);
	}

	/** Generate a QR PNG for the given string and inject it into the file input. */
	async uploadQrCodeImage(value: string): Promise<void> {
		const pngBase64 = (
			await QRCode.toBuffer(value, { type: 'png', errorCorrectionLevel: 'L' })
		).toString('base64');
		await this.agent.execute(
			(base64: string, testid: string) => {
				const binary = atob(base64);
				const bytes = new Uint8Array(binary.length);
				for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
				const file = new File([bytes], 'qr.png', { type: 'image/png' });
				const dt = new DataTransfer();
				dt.items.add(file);
				const input = document.querySelector(
					`[data-testid="${testid}"]`,
				) as HTMLInputElement | null;
				if (!input) throw new Error('file input not found');
				input.files = dt.files;
				input.dispatchEvent(new Event('change', { bubbles: true }));
			},
			pngBase64,
			FILE_INPUT_TESTID,
		);
	}

	/** Inject a blank PNG (no QR code) into the file input. */
	async uploadEmptyImage(): Promise<void> {
		await this.agent.execute(async (testid: string) => {
			const canvas = document.createElement('canvas');
			canvas.width = 64;
			canvas.height = 64;
			const ctx = canvas.getContext('2d');
			if (!ctx) throw new Error('canvas context failed');
			ctx.fillStyle = '#ffffff';
			ctx.fillRect(0, 0, 64, 64);
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
				`[data-testid="${testid}"]`,
			) as HTMLInputElement | null;
			if (!input) throw new Error('file input not found');
			input.files = dt.files;
			input.dispatchEvent(new Event('change', { bubbles: true }));
		}, FILE_INPUT_TESTID);
	}
}
