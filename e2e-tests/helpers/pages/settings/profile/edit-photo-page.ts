import { TestPage } from '../../test-page';

export class EditPhotoPage extends TestPage {
	back = this.el('edit-photo-back');
	close = this.el('edit-photo-close');
	saveButton = this.el('edit-photo-save-btn');

	async ready() {
		await this.close.waitForExist();
	}

	async save() {
		await this.saveButton.click();
	}
}
