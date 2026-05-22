import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class NewMessagePage extends TestPage {
	back = this.agent.$(tid('new-message-back'));
	search = this.agent.$(tid('new-message-search'));
	addContact = this.agent.$(`${tid('new-message-add-contact')} a`);
	contactList = this.agent.$(tid('new-message-contact-list'));

	async ready() {
		await this.addContact.waitForExist();
	}
}
