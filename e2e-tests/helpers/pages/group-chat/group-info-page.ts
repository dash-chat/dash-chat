import { Avatar } from '../../components/avatar';
import { tid } from '../../selectors';
import { TestHelper } from '../test-helper';

export class GroupInfoPage extends TestHelper {
	back = this.el(tid('group-info-back'));
	avatar = new Avatar(this.agent, 'group-info-avatar');
	headerName = this.el(tid('group-info-header-name'));
	name = this.el(tid('group-info-name'));
	description = this.el(tid('group-info-description'));
	addMembersLink = this.el(tid('group-info-add-members'));
	editLink = this.el(tid('group-info-edit-link'));
	leaveButton = this.el(tid('group-info-leave'));
	leaveSelfButton = this.el(tid('group-info-leave-self'));
	leaveConfirmButton = this.el(tid('group-info-leave-confirm'));
	leaveCancelButton = this.el(tid('group-info-leave-cancel'));
	removeMemberButton = this.el(tid('group-info-remove-member'));
	removeMemberConfirmButton = this.el(tid('group-info-remove-member-confirm'));

	memberItem(name: string) {
		return this.agent.$(tid(`group-info-member-${name}`));
	}

	async ready() {
		await this.back.waitForExist();
	}

	/** Scrolls in page JS because WebDriver's `scrollIntoView` doesn't scroll
	 * the desktop e2e webview. */
	async scrollToBottom(): Promise<void> {
		await this.agent.execute((sel: string) => {
			const page = document.querySelector(sel) as HTMLElement | null;
			if (!page) throw new Error('scrollToBottom: group info page not found');
			page.scrollTop = page.scrollHeight;
			page.dispatchEvent(new Event('scroll'));
		}, tid('group-info-page'));
	}
}
