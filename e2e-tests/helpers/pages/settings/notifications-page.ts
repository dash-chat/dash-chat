import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class NotificationsPage extends TestPage {
	back = this.agent.$(tid('notifications-back'));
	toggle = this.agent.$(tid('notifications-toggle'));

	async ready() {
		await this.toggle.waitForExist();
	}
}
