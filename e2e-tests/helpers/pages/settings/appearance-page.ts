import { TestPage } from '../test-page';

export class AppearancePage extends TestPage {
	back = this.el('appearance-back');
	light = this.el('appearance-light');
	dark = this.el('appearance-dark');
	system = this.el('appearance-system');

	async ready() {
		await this.light.waitForExist();
	}
}
