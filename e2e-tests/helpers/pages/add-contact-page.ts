import { tid } from '../../../ui/tests/selectors';
import { TestPage } from './test-page';

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
	async getContactCode(): Promise<string | null> {
		await this.qrCode.waitForExist();
		return (await this.qrCode.getProperty('value')) as string | null;
	}

	async enterCode(code: string) {
		const selector = `${tid('add-contact-code-input')} input`;
		await this.agent.$(selector).waitForExist();
		await this.agent.execute(
			(sel: string, value: string) => {
				const el = document.querySelector(sel) as HTMLInputElement;
				const setter = Object.getOwnPropertyDescriptor(
					HTMLInputElement.prototype,
					'value',
				)!.set!;
				setter.call(el, value);
				el.dispatchEvent(new Event('input', { bubbles: true }));
				el.dispatchEvent(new Event('change', { bubbles: true }));
			},
			selector,
			code,
		);
	}
}
