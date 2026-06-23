import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditNamePage extends TestPage {
	back = this.el('edit-name-back');
	nameInput = this.el('edit-name-name');
	surnameInput = this.el('edit-name-surname');
	saveButton = this.el('edit-name-save-btn');

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
