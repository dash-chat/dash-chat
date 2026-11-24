import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class DeveloperPage extends TestHelper {
	back = this.el(tid('developer-back'));
	page = this.el(tid('developer-page'));

	async ready() {
		await this.page.waitForExist();
	}
}
