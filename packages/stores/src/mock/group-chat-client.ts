import type { IGroupChatClient } from '../group-chats/group-chat-client';
import type { AgentId, PublicKey } from '../p2panda/types';
import type { ChatId, MessageContent } from '../types';

export class MockGroupChatClient implements IGroupChatClient {
	async addMember(_chatId: ChatId, _member: PublicKey): Promise<void> {}
	async removeMember(_chatId: ChatId, _member: PublicKey): Promise<void> {}
	async promoteToAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async demoteFromAdministrator(
		_chatId: ChatId,
		_member: AgentId,
	): Promise<void> {}
	async sendMessage(_chatId: ChatId, _content: MessageContent): Promise<void> {}
	async leaveGroup(): Promise<void> {}
	async deleteGroup(): Promise<void> {}
}
