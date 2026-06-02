import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class ContactUsPage extends TestPage {
	back = this.agent.$(tid('contact-us-back'));
	messageInput = this.agent.$(tid('contact-us-message-input'));
	reasonSelect = this.agent.$(tid('contact-us-reason-select'));
	includeDebugLog = this.agent.$(tid('contact-us-include-debug-log'));
	nextButton = this.agent.$(tid('contact-us-next-btn'));

	async ready() {
		await this.messageInput.waitForExist();
	}
}
