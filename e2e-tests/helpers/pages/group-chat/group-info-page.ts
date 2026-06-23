import { tid } from '../../selectors';
import { TestPage } from '../test-page';

export class GroupInfoPage extends TestPage {
	back = this.el('group-info-back');
	addMembersLink = this.el('group-info-add-members');
	editLink = this.el('group-info-edit-link');
	leaveButton = this.el('group-info-leave');
	leaveSelfButton = this.el('group-info-leave-self');
	leaveConfirmButton = this.el('group-info-leave-confirm');
	removeMemberButton = this.el('group-info-remove-member');
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
