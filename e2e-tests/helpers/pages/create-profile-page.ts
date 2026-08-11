import { tid } from '../selectors';
import { TestHelper } from './test-helper';
import { WelcomePage } from './welcome-page';

export class CreateProfilePage extends TestHelper {
	nameInput = this.el(tid('create-profile-name'));
	surnameInput = this.el(tid('create-profile-surname'));
	createButton = this.el(tid('create-profile-create-btn'));
	back = this.el(tid('create-profile-back'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	nameInputIsFocused(): Promise<boolean> {
		return this.agent.execute(
			(sel: string) => document.activeElement === document.querySelector(sel),
			`${tid('create-profile-name')} input`,
		);
	}

	async typeName(name: string, surname: string) {
		await this.typeInto(`${tid('create-profile-name')} input`, name);
		await this.typeInto(`${tid('create-profile-surname')} input`, surname);
	}

	/** Walk the whole first-launch flow: welcome screen → profile → chat list. */
	async createProfile(name: string, surname: string) {
		await new WelcomePage(this.agent).tapContinue();
		await this.ready();
		await this.typeName(name, surname);
		await this.createButton.click();
		await this.agent.$(tid('all-chats-empty')).waitForExist();
	}
}
