import { tid } from '../selectors';
import { TestHelper } from './test-helper';

export class CreateProfilePage extends TestHelper {
	nameInput = this.el(tid('create-profile-name'));
	surnameInput = this.el(tid('create-profile-surname'));
	createButton = this.el(tid('create-profile-create-btn'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	async createProfile(name: string, surname: string) {
		await this.ready();
		await this.typeInto(`${tid('create-profile-name')} input`, name);
		await this.typeInto(`${tid('create-profile-surname')} input`, surname);
		await this.createButton.click();
		await this.agent.$(tid('all-chats-empty')).waitForExist();
	}
}
