import { tid } from '../selectors';
import { TestPage } from './test-page';

export class CreateProfilePage extends TestPage {
	nameInput = this.el('create-profile-name');
	surnameInput = this.el('create-profile-surname');
	createButton = this.el('create-profile-create-btn');

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
