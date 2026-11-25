import type {
	GroupMember,
	IGroupChatClient,
} from '../group-chats/group-chat-client';
import type { AgentId } from '../p2panda/types';
import type { ChatId, GroupInfo } from '../types';
import { failIfTold } from './backend-state';
import { mockGroupMembers } from './group-chats';

export class MockGroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
		return mockGroupMembers(chatId);
	}
	onGroupMembersChanged(): () => void {
		return () => {};
	}
	async addMember(_chatId: ChatId, _member: AgentId): Promise<void> {}
	async removeMember(_chatId: ChatId, _member: AgentId): Promise<void> {}
	async promoteToAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async setInfo(_chatId: ChatId, _info: GroupInfo): Promise<void> {}
	async leaveGroup(): Promise<void> {
		failIfTold('leave_group');
	}
	async deleteGroup(): Promise<void> {}
}
