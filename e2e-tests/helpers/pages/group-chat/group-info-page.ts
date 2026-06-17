import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupInfoPage extends TestPage {
	back = this.agent.$(tid('group-info-back'));
	addMembersLink = this.agent.$(tid('group-info-add-members'));
	editLink = this.agent.$(tid('group-info-edit-link'));
	leaveButton = this.agent.$(tid('group-info-leave'));
	leaveSelfButton = this.agent.$(tid('group-info-leave-self'));
	leaveConfirmButton = this.agent.$(tid('group-info-leave-confirm'));
	removeMemberButton = this.agent.$(tid('group-info-remove-member'));
	removeMemberConfirmButton = this.agent.$(
		tid('group-info-remove-member-confirm'),
	);

	memberItem(name: string) {
		return this.agent.$(tid(`group-info-member-${name}`));
	}

	async ready() {
		await this.back.waitForExist();
	}
}
