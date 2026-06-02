import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class NewGroupPage extends TestPage {
	back = this.agent.$(tid('new-group-back'));
	membersNavbar = this.agent.$(tid('new-group-members-navbar'));
	nextButton = this.agent.$(tid('new-group-next'));
	infoNavbar = this.agent.$(tid('new-group-info-navbar'));
	infoBack = this.agent.$(tid('new-group-info-back'));
	nameInput = this.agent.$(tid('new-group-name-input'));
	createButton = this.agent.$(tid('new-group-create'));

	async ready() {
		await this.membersNavbar.waitForExist();
	}

	async readyOnInfo() {
		await this.infoNavbar.waitForExist();
	}

	async selectMember(name: string) {
		const item = this.agent.$(
			`[data-testid="selectable-contact-item"][data-contact-name="${name}"]`,
		);
		await item.waitForExist();
		await item.click();
	}

	async next() {
		await this.nextButton.click();
	}

	async setName(name: string) {
		await this.typeInto(`${tid('new-group-name-input')} input`, name);
	}

	async create() {
		await this.createButton.click();
	}
}
