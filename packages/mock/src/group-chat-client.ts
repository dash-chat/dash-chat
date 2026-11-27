import type {
	AgentId,
	ChatId,
	GroupInfo,
	GroupMember,
	IGroupChatClient,
} from 'dash-chat-stores';

import { failIfTold } from './backend-state';
import { mockGroupMembers, onMockGroupMembersChanged } from './group-chats';

export class MockGroupChatClient implements IGroupChatClient {
	async getMembers(chatId: ChatId): Promise<GroupMember[]> {
		return mockGroupMembers(chatId);
	}
	onGroupMembersChanged(chatId: ChatId, handler: () => void): () => void {
		return onMockGroupMembersChanged(chatId, handler);
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
