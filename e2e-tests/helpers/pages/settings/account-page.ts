import { TestPage } from '../test-page';

export class AccountPage extends TestPage {
	back = this.el('account-back');
	deleteItem = this.el('account-delete');
	deleteConfirm = this.el('account-delete-confirm');
	deleteCancel = this.el('account-delete-cancel');

	async ready() {
		await this.deleteItem.waitForExist();
	}
}
