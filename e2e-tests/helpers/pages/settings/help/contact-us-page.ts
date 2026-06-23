import { TestPage } from '../../test-page';

export class ContactUsPage extends TestPage {
	back = this.el('contact-us-back');
	messageInput = this.el('contact-us-message-input');
	reasonSelect = this.el('contact-us-reason-select');
	includeDebugLog = this.el('contact-us-include-debug-log');
	nextButton = this.el('contact-us-next-btn');

	async ready() {
		await this.messageInput.waitForExist();
	}
}
