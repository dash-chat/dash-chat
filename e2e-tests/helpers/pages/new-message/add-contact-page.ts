import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class AddContactPage extends TestPage {
	back = this.agent.$(tid('add-contact-back'));
	codeTab = this.agent.$(tid('add-contact-code-tab'));
	scanTab = this.agent.$(tid('add-contact-scan-tab'));
	qrCode = this.agent.$('wa-qr-code');
	copyButton = this.agent.$(tid('add-contact-copy-btn'));
	codeInput = this.agent.$(tid('add-contact-code-input'));
	shareButton = this.agent.$(tid('add-contact-share-btn'));
	saveButton = this.agent.$(tid('add-contact-save-btn'));
	uploadButton = this.agent.$(tid('add-contact-upload-btn'));
	selectImageButton = this.agent.$(tid('add-contact-select-image-btn'));
	fileInput = this.agent.$(tid('add-contact-file-input'));
	colorButton = this.agent.$(tid('add-contact-color-btn'));

	async ready() {
		await this.codeInput.waitForExist();
	}

	/** Read the contact code from the QR element. */
	async getContactCode(): Promise<string> {
		await this.qrCode.waitForExist();
		const code = (await this.qrCode.getProperty('value')) as string | null;
		if (!code) throw new Error('contact code missing on QR element');
		return code;
	}

	async enterCode(code: string) {
		await this.typeInto(`${tid('add-contact-code-input')} input`, code);
	}

	/** Generate a QR PNG for the given code and inject it into the file input. */
	async uploadQrCodeImage(code: string): Promise<void> {
		await this.agent.execute(
			(c: string) => window.__test.uploadQrCodeImage(c),
			code,
		);
	}

	/** Inject a blank PNG (no QR code) into the file input. */
	async uploadEmptyImage(): Promise<void> {
		await this.agent.execute(() => window.__test.uploadEmptyImage());
	}
}
