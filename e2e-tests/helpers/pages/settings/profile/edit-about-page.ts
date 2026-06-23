import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class EditAboutPage extends TestHelper {
	back = this.el(tid('edit-about-back'));
	input = this.el(tid('edit-about-input'));
	saveButton = this.el(tid('edit-about-save-btn'));

	async ready() {
		await this.input.waitForExist();
	}

	async setAbout(text: string) {
		await this.typeInto(`${tid('edit-about-input')} textarea`, text);
	}

	async save() {
		await this.saveButton.click();
	}
}
