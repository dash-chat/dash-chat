import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class AddMembersStep extends TestPage {
	back = this.el('new-group-back');
	navbar = this.el('new-group-members-navbar');
	nextButton = this.el('new-group-next');

	async ready() {
		await this.navbar.waitForExist();
	}

	async addContactByName(name: string) {
		const item = this.agent.$(
			`[data-testid="selectable-contact-item"][data-contact-name="${name}"]`,
		);
		await item.waitForExist();
		await item.click();
	}
}

export class GroupInfoStep extends TestPage {
	navbar = this.el('new-group-info-navbar');
	back = this.el('new-group-info-back');
	nameInput = this.el('new-group-name-input');
	createButton = this.el('new-group-create');

	async ready() {
		await this.navbar.waitForExist();
	}

	async setName(name: string) {
		await this.typeInto(tid('new-group-name-input'), name);
	}
}

export class NewGroupPage extends TestPage {
	addMembersStep = new AddMembersStep(this.agent);
	groupInfoStep = new GroupInfoStep(this.agent);

	async ready() {
		await this.addMembersStep.ready();
	}
}
