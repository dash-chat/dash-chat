import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class AccountPage extends TestPage {
	back = this.agent.$(tid('account-back'));
	deleteItem = this.agent.$(tid('account-delete'));
	deleteConfirm = this.agent.$(tid('account-delete-confirm'));
	deleteCancel = this.agent.$(tid('account-delete-cancel'));

	async ready() {
		await this.deleteItem.waitForExist();
	}
}
