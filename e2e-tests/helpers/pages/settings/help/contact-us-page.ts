import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class ContactUsPage extends TestHelper {
	back = this.el(tid('contact-us-back'));
	messageInput = this.el(tid('contact-us-message-input'));
	reasonSelect = this.el(tid('contact-us-reason-select'));
	includeDebugLog = this.el(tid('contact-us-include-debug-log'));
	sendButton = this.el(tid('contact-us-send-btn'));

	async ready() {
		await this.messageInput.waitForExist();
	}

	async selectReason(reason: string) {
		await this.agent
			.$(`${tid('contact-us-reason-select')} select`)
			.selectByAttribute('value', reason);
	}

	async enterMessage(message: string) {
		await this.typeInto(`${tid('contact-us-message-input')} textarea`, message);
	}
}
