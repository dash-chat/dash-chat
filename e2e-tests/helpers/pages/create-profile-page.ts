import { tid } from '../../../ui/tests/selectors';
import { TestPage } from './test-page';

export class CreateProfilePage extends TestPage {
	nameInput = this.agent.$(tid('create-profile-name'));
	surnameInput = this.agent.$(tid('create-profile-surname'));
	createButton = this.agent.$(tid('create-profile-create-btn'));
	createLink = this.agent.$(tid('create-profile-create-link'));

	async ready() {
		await this.nameInput.waitForExist();
	}

	async createProfile(name: string, surname: string) {
		await this.ready();
		await this.typeInto(`${tid('create-profile-name')} input`, name);
		await this.typeInto(`${tid('create-profile-surname')} input`, surname);
		const submit = (await this.createButton.isExisting())
			? this.createButton
			: this.createLink;
		await submit.click();
		await this.agent.$(tid('all-chats-empty')).waitForExist();
	}

	private async typeInto(selector: string, value: string) {
		await this.agent.$(selector).waitForExist();
		await this.agent.execute(
			(sel: string, val: string) => {
				const el = document.querySelector(sel) as
					| HTMLInputElement
					| HTMLTextAreaElement;
				const proto =
					el.tagName === 'TEXTAREA'
						? HTMLTextAreaElement.prototype
						: HTMLInputElement.prototype;
				const setter = Object.getOwnPropertyDescriptor(proto, 'value')!.set!;
				setter.call(el, val);
				el.dispatchEvent(new Event('input', { bubbles: true }));
				el.dispatchEvent(new Event('change', { bubbles: true }));
			},
			selector,
			value,
		);
	}
}
