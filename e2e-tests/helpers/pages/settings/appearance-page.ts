import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class AppearancePage extends TestPage {
	back = this.agent.$(tid('appearance-back'));
	light = this.agent.$(tid('appearance-light'));
	dark = this.agent.$(tid('appearance-dark'));
	system = this.agent.$(tid('appearance-system'));

	async ready() {
		await this.light.waitForExist();
	}
}
