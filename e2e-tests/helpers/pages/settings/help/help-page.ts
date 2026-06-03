import { tid } from '../../../selectors';
import { TestPage } from '../../test-page';

export class HelpPage extends TestPage {
	back = this.agent.$(tid('help-back'));
	contactUsLink = this.agent.$(tid('help-contact-us'));
	versionItem = this.agent.$(tid('help-version'));
	previewFeaturesToggle = this.agent.$(tid('help-preview-features-toggle'));

	async ready() {
		await this.contactUsLink.waitForExist();
	}
}
