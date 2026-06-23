import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class GroupInfoEditPage extends TestHelper {
	nameInput = this.el(tid('group-info-edit-name'));
	saveButton = this.el(tid('group-info-edit-save-btn'));

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
