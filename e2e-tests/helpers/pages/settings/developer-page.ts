import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class DeveloperPage extends TestHelper {
	back = this.el(tid('developer-back'));
	disable = this.el(tid('developer-disable'));

	async ready() {
		await this.disable.waitForExist();
	}
}
