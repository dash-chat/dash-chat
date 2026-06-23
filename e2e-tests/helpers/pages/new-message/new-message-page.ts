import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class NewMessagePage extends TestHelper {
	back = this.el(tid('new-message-back'));
	search = this.el(tid('new-message-search'));
	addContact = this.el(`${tid('new-message-add-contact')} a`);
	newGroup = this.el(`${tid('new-message-new-group')} a`);
	contactList = this.el(tid('new-message-contact-list'));

	async ready() {
		await this.addContact.waitForExist();
	}
}
