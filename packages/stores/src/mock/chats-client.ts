import type { IChatsClient } from '../chats/chats-client';
import type { Hash } from '../p2panda/types';
import type { ChatId } from '../types';

export class MockChatsClient implements IChatsClient {
	private groupChats: ChatId[] = [];

	async createGroupChat(chatId: ChatId): Promise<void> {
		this.groupChats.push(chatId);
	}

	async getGroupChats(): Promise<ChatId[]> {
		return this.groupChats;
	}

	async markMessagesRead(
		_chatId: ChatId,
		_messageHashes: Hash[],
	): Promise<void> {}
}
