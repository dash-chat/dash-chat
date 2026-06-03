import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditAboutPage extends TestPage {
	back = this.agent.$(tid('edit-about-back'));
	input = this.agent.$(tid('edit-about-input'));
	saveButton = this.agent.$(tid('edit-about-save-btn'));

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
