import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class EditPhotoPage extends TestPage {
	back = this.agent.$(tid('edit-photo-back'));
	close = this.agent.$(tid('edit-photo-close'));
	saveButton = this.agent.$(tid('edit-photo-save-btn'));
	textButton = this.agent.$(tid('edit-photo-text'));
	textPreview = this.agent.$(tid('text-avatar-preview'));

	async ready() {
		await this.close.waitForExist();
	}

	async save() {
		await this.saveButton.click();
	}

	/** The text-avatar editor's current text and preview background. */
	async textAvatarState(): Promise<{ text: string; backgroundColor: string }> {
		return this.agent.execute(
			(inputSel: string, previewSel: string) => {
				const input = document.querySelector(inputSel) as HTMLInputElement;
				const preview = document.querySelector(previewSel) as HTMLElement;
				return {
					text: input.value,
					backgroundColor: getComputedStyle(preview).backgroundColor,
				};
			},
			tid('text-avatar-input'),
			tid('text-avatar-preview'),
		);
	}
}
