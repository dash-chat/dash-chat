import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class GroupInfoPage extends TestHelper {
	back = this.el(tid('group-info-back'));
	addMembersLink = this.el(tid('group-info-add-members'));
	editLink = this.el(tid('group-info-edit-link'));
	leaveButton = this.el(tid('group-info-leave'));
	leaveSelfButton = this.el(tid('group-info-leave-self'));
	leaveConfirmButton = this.el(tid('group-info-leave-confirm'));
	removeMemberButton = this.el(tid('group-info-remove-member'));
	removeMemberConfirmButton = this.el(
		tid('group-info-remove-member-confirm'),
	);

	memberItem(name: string) {
		return this.agent.$(tid(`group-info-member-${name}`));
	}

	async ready() {
		await this.back.waitForExist();
	}
}
