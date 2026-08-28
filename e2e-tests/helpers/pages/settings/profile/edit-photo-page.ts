import type {
	FilePickerRequest,
	TestFileSpec,
} from '../../../../../ui/tests/setup-utils';
import { Avatar } from '../../../components/avatar';
import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class EditPhotoPage extends TestHelper {
	back = this.el(tid('edit-photo-back'));
	avatar = new Avatar(this.agent, 'avatar-preview');
	close = this.el(tid('edit-photo-close'));
	saveButton = this.el(tid('edit-photo-save-btn'));
	cameraButton = this.el(tid('edit-photo-camera'));
	galleryButton = this.el(tid('edit-photo-gallery'));

	async ready() {
		await this.close.waitForExist();
	}

	async save() {
		await this.saveButton.click();
	}

	/** Shoot `photo` with the camera action, reporting what it asked the OS for. */
	takePhoto(photo: TestFileSpec): Promise<FilePickerRequest> {
		return this.answerFilePicker(this.cameraButton, [photo]);
	}

	/** Pick `photo` with the gallery action, reporting what it asked the OS for. */
	pickPhoto(photo: TestFileSpec): Promise<FilePickerRequest> {
		return this.answerFilePicker(this.galleryButton, [photo]);
	}
}
