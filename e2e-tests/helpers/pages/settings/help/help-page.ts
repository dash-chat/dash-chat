import { tid } from '../../../selectors';
import { TestHelper } from '../../test-helper';

export class HelpPage extends TestHelper {
	back = this.el(tid('help-back'));
	contactUsLink = this.el(tid('help-contact-us'));
	versionItem = this.el(tid('help-version'));
	previewFeaturesToggle = this.el(tid('help-preview-features-toggle'));

	async ready() {
		await this.contactUsLink.waitForExist();
	}
}
