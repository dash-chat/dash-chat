import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditPhotoPage extends TestPage {
	back = this.agent.$(tid('edit-photo-back'));
	close = this.agent.$(tid('edit-photo-close'));
	saveButton = this.agent.$(tid('edit-photo-save-btn'));

	async ready() {
		await this.close.waitForExist();
	}

	async save() {
		await this.saveButton.click();
	}
}
