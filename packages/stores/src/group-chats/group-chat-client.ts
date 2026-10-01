import { type UnsubscribeFunction } from 'emittery';

import { AgentId, DeviceId } from '../p2panda/types';
import { ChatId, GroupInfo } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';
import { onSystemEvent } from '../utils/system-events';

export interface GroupDevice {
	deviceId: DeviceId;
	isAdmin: boolean;
}

export interface IGroupChatClient {
	/** The devices in the group; their agents come from the contacts store. */
	getMembers(chatId: ChatId): Promise<GroupDevice[]>;
	/** `chatId`'s members changed, here or in the iOS push extension. */
	onGroupMembersChanged(
		chatId: ChatId,
		handler: () => void,
	): UnsubscribeFunction;
	addMember(chatId: ChatId, member: AgentId): Promise<void>;
	removeMember(chatId: ChatId, member: AgentId): Promise<void>;

	promoteToAdministrator(chatId: ChatId, member: AgentId): Promise<void>;
	demoteFromAdministrator(chatId: ChatId, member: AgentId): Promise<void>;

	setInfo(chatId: ChatId, info: GroupInfo): Promise<void>;

	leaveGroup(chatId: ChatId): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupDevice[]> {
		return invokeAfterSetup('get_group_members', { chatId });
	}

	async addMember(chatId: ChatId, member: AgentId): Promise<void> {
		await invokeAfterSetup('add_group_member', { chatId, agentId: member });
	}
	async removeMember(chatId: ChatId, member: AgentId): Promise<void> {
		await invokeAfterSetup('remove_group_member', { chatId, agentId: member });
	}

	setInfo(chatId: ChatId, info: GroupInfo): Promise<void> {
		return invokeAfterSetup('set_group_info', { chatId, info });
	}
	async promoteToAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		chatId: ChatId,
		member: AgentId,
	): Promise<void> {}

	async leaveGroup(chatId: ChatId): Promise<void> {
		await invokeAfterSetup('leave_group', { chatId });
	}

	async deleteGroup(): Promise<void> {}

	onGroupMembersChanged(
		chatId: ChatId,
		handler: () => void,
	): UnsubscribeFunction {
		// Its creation sets the member list too, for a store that already
		// existed when the group's own operations arrived.
		return onSystemEvent(['GroupMembersChanged', 'GroupChatAdded'], event => {
			if (event.payload.chat_id === chatId) handler();
		});
	}
}
