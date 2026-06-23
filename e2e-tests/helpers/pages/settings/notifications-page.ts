import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class NotificationsPage extends TestHelper {
	back = this.el(tid('notifications-back'));
	toggle = this.el(tid('notifications-toggle'));

	async ready() {
		await this.toggle.waitForExist();
	}
}
