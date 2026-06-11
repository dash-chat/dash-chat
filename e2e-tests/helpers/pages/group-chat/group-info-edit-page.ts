import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupInfoEditPage extends TestPage {
	nameInput = this.agent.$(tid('group-info-edit-name'));
	saveButton = this.agent.$(tid('group-info-edit-save-btn'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	async setName(name: string) {
		await this.typeInto(`${tid('group-info-edit-name')} input`, name);
	}

	async save() {
		await this.saveButton.click();
	}
}
