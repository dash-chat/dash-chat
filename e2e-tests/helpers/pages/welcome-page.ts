import { tid } from '../selectors';
import { TestHelper } from './test-helper';

export class WelcomePage extends TestHelper {
	title = this.el(tid('welcome-title'));
	termsLink = this.el(tid('welcome-terms-link'));
	continueButton = this.el(tid('welcome-continue-btn'));

	async ready() {
		await this.title.waitForExist();
	}

	async tapContinue() {
		await this.ready();
		await this.continueButton.click();
	}
}
