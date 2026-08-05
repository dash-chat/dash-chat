import type {
	FilePickerAttempt,
	TestFileSpec,
} from '../../../../../ui/tests/setup-utils';
import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class EditPhotoPage extends TestHelper {
	back = this.el(tid('edit-photo-back'));
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

	/**
	 * Run `open`, answer the file input it opens with `files`, and report what
	 * that input asked the OS for. No native picker or camera appears; with no
	 * `files` the input is dismissed instead.
	 */
	async pickerOpenedBy(
		open: () => Promise<void>,
		files: TestFileSpec[] = [],
	): Promise<FilePickerAttempt> {
		await this.agent.execute(
			(specs: TestFileSpec[]) => window.__test.interceptFilePickers(specs),
			files,
		);
		let failure: unknown;
		try {
			await open();
		} catch (error) {
			failure = error;
		}
		const attempts = await this.agent.execute(() =>
			window.__test.collectFilePickers(),
		);
		if (failure) throw failure;
		expect(attempts).toHaveLength(1);
		return attempts[0];
	}
}
