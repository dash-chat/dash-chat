import { TestPage } from '../test-page';

export class AddMembersPage extends TestPage {
	back = this.el('add-members-back');
	addButton = this.el('add-members-add-btn');

	async ready() {
		await this.back.waitForExist();
	}

	async addContactByName(name: string) {
		const item = this.agent.$(
			`[data-testid="selectable-contact-item"][data-contact-name="${name}"]`,
		);
		await item.waitForExist();
		await item.click();
	}
}
