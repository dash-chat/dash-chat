import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class GroupInfoEditPage extends TestHelper {
	nameInput = this.el(tid('group-info-edit-name'));
	descriptionInput = this.el(tid('group-info-edit-description'));
	editPhotoButton = this.el(tid('edit-photo'));
	saveButton = this.el(tid('group-info-edit-save-btn'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	async setName(name: string) {
		await this.typeInto(`${tid('group-info-edit-name')} input`, name);
	}

	async setDescription(description: string) {
		await this.typeInto(
			`${tid('group-info-edit-description')} textarea`,
			description,
		);
	}

	async save() {
		await this.saveButton.click();
	}
}
