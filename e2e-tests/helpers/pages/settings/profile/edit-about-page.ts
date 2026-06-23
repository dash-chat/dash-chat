import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditAboutPage extends TestPage {
	back = this.el('edit-about-back');
	input = this.el('edit-about-input');
	saveButton = this.el('edit-about-save-btn');

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
