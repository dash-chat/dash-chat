import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class NewGroupPage extends TestPage {
	back = this.agent.$(tid('new-group-back'));
	nextButton = this.agent.$(tid('new-group-next-btn'));
	infoBack = this.agent.$(tid('new-group-info-back'));
	nameInput = this.agent.$(tid('new-group-name-input'));
	createButton = this.agent.$(tid('new-group-create-btn'));

	async ready() {
		await this.nextButton.waitForExist();
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
