import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class NewMessagePage extends TestPage {
	back = this.el('new-message-back');
	search = this.el('new-message-search');
	addContact = this.agent.$(`${tid('new-message-add-contact')} a`);
	newGroup = this.agent.$(`${tid('new-message-new-group')} a`);
	contactList = this.el('new-message-contact-list');

	async ready() {
		await this.addContact.waitForExist();
	}
}
