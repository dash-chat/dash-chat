import type {
	GroupMember,
	IGroupChatClient,
} from '../group-chats/group-chat-client';
import type { AgentId } from '../p2panda/types';
import type { ChatId, GroupInfo } from '../types';

export class MockGroupChatClient implements IGroupChatClient {
	async getMembers(_chatId: ChatId): Promise<GroupMember[]> {
		return [];
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
	async leaveGroup(): Promise<void> {}
	async deleteGroup(): Promise<void> {}
}
