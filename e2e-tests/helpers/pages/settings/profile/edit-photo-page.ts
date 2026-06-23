import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class EditPhotoPage extends TestHelper {
	back = this.el(tid('edit-photo-back'));
	close = this.el(tid('edit-photo-close'));
	saveButton = this.el(tid('edit-photo-save-btn'));

	async ready() {
		await this.close.waitForExist();
	}

	async save() {
		await this.saveButton.click();
	}
}
