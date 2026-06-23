import { TestPage } from '../test-page';

export class NotificationsPage extends TestPage {
	back = this.el('notifications-back');
	toggle = this.el('notifications-toggle');

	async ready() {
		await this.toggle.waitForExist();
	}
}
