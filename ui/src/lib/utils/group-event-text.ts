import { m } from '$lib/paraglide/messages.js';
import type { GroupControlEvent } from 'dash-chat-stores';

export function groupEventText(event: GroupControlEvent): string {
	switch (event.kind) {
		case 'group_created':
			if (event.isMine) return m.youCreatedTheGroup();
			if (event.iAmInitialMember) {
				return m.someoneAddedYouToTheGroup({
					name: event.creatorName || m.someone(),
				});
			}
			if (event.creatorName) {
				return m.someoneCreatedTheGroup({ name: event.creatorName });
			}
			return m.groupCreated();
		case 'group_member_added':
			if (event.isMine) {
				return m.someoneAddedYouToTheGroup({
					name: event.adminName || m.someone(),
				});
			}
			if (event.addedByMe) {
				return m.youAddedMember({ name: event.memberName || m.someone() });
			}
			return m.memberAddedToGroup({
				admin: event.adminName || m.someone(),
				name: event.memberName || m.someone(),
			});
		case 'group_member_removed':
			if (event.isMine) {
				return m.someoneRemovedYouFromTheGroup({
					name: event.adminName || m.someone(),
				});
			}
			if (event.removedByMe) {
				return m.youRemovedMember({ name: event.memberName || m.someone() });
			}
			if (event.memberName || event.adminName) {
				return m.memberRemovedFromGroupBy({
					admin: event.adminName || m.someone(),
					name: event.memberName || m.someone(),
				});
			}
			return m.memberRemovedFromGroup();
		case 'group_member_promoted':
			if (event.promotedByMe) {
				return m.youMadeMemberAdmin({ name: event.memberName || m.someone() });
			}
			return m.someoneMadeMemberAdmin({
				admin: event.adminName || m.someone(),
				name: event.memberName || m.someone(),
			});
		case 'group_member_demoted':
			if (event.demotedByMe) {
				return m.youRevokedAdminFromMember({
					name: event.memberName || m.someone(),
				});
			}
			return m.someoneRevokedAdminFromMember({
				admin: event.adminName || m.someone(),
				name: event.memberName || m.someone(),
			});
	}
}
