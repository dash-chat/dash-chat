import { AgentId, DeviceId } from '../p2panda/types';
import { ChatId, GroupInfo } from '../types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface GroupMember {
	agentId: AgentId;
	deviceIds: DeviceId[];
	isAdmin: boolean;
}

export interface IGroupChatClient {
	getMembers(chatId: ChatId): Promise<GroupMember[]>;
	addMember(chatId: ChatId, member: AgentId): Promise<void>;
	removeMember(chatId: ChatId, member: AgentId): Promise<void>;

	promoteToAdministrator(chatId: ChatId, member: AgentId): Promise<void>;
	demoteFromAdministrator(chatId: ChatId, member: AgentId): Promise<void>;

	setInfo(chatId: ChatId, info: GroupInfo): Promise<void>;

	leaveGroup(chatId: ChatId): Promise<void>;
	deleteGroup(): Promise<void>;
}

export class GroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
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
}
