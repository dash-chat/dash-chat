import { SelectableContactList } from '../../components/selectable-contact-list';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class AddMembersStep extends TestHelper {
	back = this.el(tid('new-group-back'));
	navbar = this.el(tid('new-group-members-navbar'));
	nextButton = this.el(tid('new-group-next'));
	contactList = new SelectableContactList(this.agent);

	async ready() {
		await this.navbar.waitForExist();
	}

	async addContactByName(name: string) {
		const item = this.contactList.contactItem(name);
		await item.waitForExist();
		await item.click();
	}
}

export class GroupInfoStep extends TestHelper {
	navbar = this.el(tid('new-group-info-navbar'));
	back = this.el(tid('new-group-info-back'));
	nameInput = this.el(tid('new-group-name-input'));
	createButton = this.el(tid('new-group-create'));

	async ready() {
		await this.navbar.waitForExist();
	}

	async setName(name: string) {
		await this.typeInto(tid('new-group-name-input'), name);
	}
}

export class NewGroupPage extends TestHelper {
	addMembersStep = new AddMembersStep(this.agent);
	groupInfoStep = new GroupInfoStep(this.agent);

	async ready() {
		await this.addMembersStep.ready();
	}
}
