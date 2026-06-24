import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class EditNamePage extends TestHelper {
	back = this.el(tid('edit-name-back'));
	nameInput = this.el(tid('edit-name-name'));
	surnameInput = this.el(tid('edit-name-surname'));
	saveButton = this.el(tid('edit-name-save-btn'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	async setName(name: string, surname?: string) {
		await this.typeInto(`${tid('edit-name-name')} input`, name);
		if (surname !== undefined) {
			await this.typeInto(`${tid('edit-name-surname')} input`, surname);
		}
	}

	async save() {
		await this.saveButton.click();
	}
}
