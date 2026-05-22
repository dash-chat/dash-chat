import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditNamePage extends TestPage {
	back = this.agent.$(tid('edit-name-back'));
	nameInput = this.agent.$(tid('edit-name-name'));
	surnameInput = this.agent.$(tid('edit-name-surname'));
	saveButton = this.agent.$(tid('edit-name-save-btn'));

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
