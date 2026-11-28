import QRCode from 'qrcode';

import { TINY_PNG_BYTES } from '../../images';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

const FILE_INPUT_TESTID = 'add-contact-file-input';

export class AddContactPage extends TestHelper {
	back = this.el(tid('add-contact-back'));
	codeTab = this.el(tid('add-contact-link-tab'));
	scanTab = this.el(tid('add-contact-scan-tab'));
	qrCode = this.el('wa-qr-code');
	copyLinkBox = this.el(tid('add-contact-copy-link-box'));
	copyLinkButton = this.el(tid('add-contact-copy-link-btn'));
	linkButton = this.el(tid('add-contact-link-btn'));
	linkSheet = this.el(tid('qr-link-sheet'));
	linkSheetLink = this.el(tid('qr-link-sheet-link'));
	linkSheetCopyButton = this.el(tid('qr-link-sheet-copy'));
	linkSheetShareButton = this.el(tid('qr-link-sheet-share'));
	shareButton = this.el(tid('add-contact-share-btn'));
	saveButton = this.el(tid('add-contact-save-btn'));
	uploadButton = this.el(tid('add-contact-upload-btn'));
	selectImageButton = this.el(tid('add-contact-select-image-btn'));
	fileInput = this.el(tid(FILE_INPUT_TESTID));
	colorButton = this.el(tid('add-contact-color-btn'));

	async ready() {
		await this.qrCode.waitForExist();
	}

	/** True if the link sheet is open (not the slide-out dismissed state). */
	linkSheetIsOpen(): Promise<boolean> {
		return this.agent.execute((sel: string) => {
			const inner = document.querySelector(sel);
			const sheet = inner?.closest('.k-sheet');
			if (!sheet) return false;
			return sheet.classList.contains('-translate-y-full');
		}, tid('qr-link-sheet'));
	}

	/** Close the link sheet by clicking its backdrop. */
	async closeLinkSheet() {
		await this.agent.execute((sel: string) => {
			const inner = document.querySelector(sel);
			const sheet = inner?.closest('.k-sheet');
			const backdrop = sheet?.previousElementSibling as HTMLElement | null;
			backdrop?.click();
		}, tid('qr-link-sheet'));
	}

	/** Read the contact link from the QR element. */
	async getAddContactLink(): Promise<string> {
		await this.qrCode.waitForExist();
		const link = (await this.qrCode.getProperty('value')) as string | null;
		if (!link) throw new Error('add contact link missing on QR element');
		return link;
	}

	// There is no UI to type a contact link; receiving one is only reachable by
	// opening a shared link, so dispatch it through the app's deep-link routing.
	async enterAddContactLink(link: string) {
		await this.agent.execute(
			(l: string) => window.__test.handleDeepLink(l),
			link,
		);
	}

	/** Generate a QR PNG for the given string and inject it into the file input. */
	async uploadQrCodeImage(value: string): Promise<void> {
		const png = await QRCode.toBuffer(value, {
			type: 'png',
			errorCorrectionLevel: 'L',
		});
		await this.pickImage('qr.png', Array.from(png));
	}

	/** Inject a PNG with no QR code in it into the file input. */
	async uploadEmptyImage(): Promise<void> {
		await this.pickImage('blank.png', TINY_PNG_BYTES);
	}

	private async pickImage(name: string, bytes: number[]): Promise<void> {
		await this.agent.execute(
			(testid: string, fileName: string, pngBytes: number[]) =>
				window.__test.pickFilesInto(testid, [
					{ name: fileName, mimeType: 'image/png', bytes: pngBytes },
				]),
			FILE_INPUT_TESTID,
			name,
			bytes,
		);
	}
}
