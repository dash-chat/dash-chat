import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class ContactUsPage extends TestHelper {
	back = this.el(tid('contact-us-back'));
	messageInput = this.el(tid('contact-us-message-input'));
	reasonSelect = this.el(tid('contact-us-reason-select'));
	includeDebugLog = this.el(tid('contact-us-include-debug-log'));
	nextButton = this.el(tid('contact-us-next-btn'));

	async ready() {
		await this.messageInput.waitForExist();
	}
}
