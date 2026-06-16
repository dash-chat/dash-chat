import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupInfoPage extends TestPage {
	back = this.agent.$(tid('group-info-back'));
	addMembersLink = this.agent.$(tid('group-info-add-members'));
	editLink = this.agent.$(tid('group-info-edit-link'));
	leaveButton = this.agent.$(tid('group-info-leave'));
	leaveConfirmButton = this.agent.$(tid('group-info-leave-confirm'));

	async ready() {
		await this.back.waitForExist();
	}
}
