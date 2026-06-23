import { TestPage } from '../../test-page';

export class HelpPage extends TestPage {
	back = this.el('help-back');
	contactUsLink = this.el('help-contact-us');
	versionItem = this.el('help-version');
	previewFeaturesToggle = this.el('help-preview-features-toggle');

	async ready() {
		await this.contactUsLink.waitForExist();
	}
}
