import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class AccountPage extends TestHelper {
	back = this.el(tid('account-back'));
	deleteItem = this.el(tid('account-delete'));
	deleteConfirm = this.el(tid('account-delete-confirm'));
	deleteCancel = this.el(tid('account-delete-cancel'));

	async ready() {
		await this.deleteItem.waitForExist();
	}
}
