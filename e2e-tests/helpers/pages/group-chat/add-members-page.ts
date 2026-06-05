import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class AddMembersPage extends TestPage {
	back = this.agent.$(tid('add-members-back'));
	addButton = this.agent.$(tid('add-members-add-btn'));

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
