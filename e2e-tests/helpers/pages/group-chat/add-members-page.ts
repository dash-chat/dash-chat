import { SelectableContactList } from '../../components/selectable-contact-list';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class AddMembersPage extends TestHelper {
	back = this.el(tid('add-members-back'));
	addButton = this.el(tid('add-members-add-btn'));
	contactList = new SelectableContactList(this.agent);

	async ready() {
		await this.back.waitForExist();
	}

	async addContactByName(name: string) {
		const item = this.contactList.contactItem(name);
		await item.waitForExist();
		await item.click();
	}
}
