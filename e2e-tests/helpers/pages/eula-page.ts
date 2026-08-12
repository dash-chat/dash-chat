import { tid } from '../selectors';
import { TestHelper } from './test-helper';

export class EulaPage extends TestHelper {
	body = this.el(tid('eula-body'));
	back = this.el(tid('eula-back'));

	async ready() {
		await this.body.waitForExist();
	}
}
